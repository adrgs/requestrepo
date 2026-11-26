//! The API as an agent drives it from a terminal: curl-shaped requests, long polling, and
//! the discovery documents on the main domain.

#[cfg(test)]
mod tests {
    use crate::abuse::Settings;
    use crate::tests::harness::TestServer;
    use crate::utils::config::CONFIG;
    use crate::utils::{generate_jwt, get_random_subdomain};
    use serde_json::{json, Value};
    use std::time::{Duration, Instant};

    async fn server() -> TestServer {
        TestServer::start(Settings::default()).await
    }

    fn paths(list: &Value) -> Vec<String> {
        list["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["path"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    // --- Sessions ----------------------------------------------------------

    #[tokio::test]
    async fn a_session_needs_no_body_and_says_where_to_send_traffic() {
        let server = server().await;
        // `curl -X POST .../api/v2/sessions`: no body, no Content-Type
        let reply = server.main("POST", "/api/v2/sessions", &[]).await;
        assert_eq!(reply.status, 201, "{}", reply.body);
        let session = reply.json();

        let subdomain = session["subdomain"].as_str().unwrap();
        let domain = format!("{subdomain}.{}", CONFIG.server_domain);
        assert_eq!(session["domain"], domain.as_str());
        assert_eq!(
            session["url"],
            format!("{}/", crate::http::routes_v2::public_origin(&domain)).as_str()
        );
        assert_eq!(session["email"], format!("anything@{domain}").as_str());
        assert!(session["token"].as_str().unwrap().starts_with("eyJ"));

        // An empty JSON object still works, as the dashboard sends it
        let with_body = server
            .request(
                "POST",
                &server.main_host(),
                "/api/v2/sessions",
                &[("Content-Type", "application/json")],
                "{}",
            )
            .await;
        assert_eq!(with_body.status, 201);

        let malformed = server
            .request(
                "POST",
                &server.main_host(),
                "/api/v2/sessions",
                &[("Content-Type", "application/json")],
                "{nope",
            )
            .await;
        assert_eq!(malformed.status, 400);
        assert_eq!(malformed.code(), "invalid_body");
    }

    // --- Reading captured traffic ------------------------------------------

    #[tokio::test]
    async fn after_returns_only_newer_requests_and_type_filters() {
        let server = server().await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();

        for path in ["/one", "/two"] {
            server.request("GET", &host, path, &[], "").await;
        }
        let first = server
            .api(&token, "GET", "/api/v2/requests", "")
            .await
            .json();
        assert_eq!(paths(&first), ["/one", "/two"]);
        let cursor = first["cursor"].as_str().unwrap().to_string();
        assert_eq!(cursor, first["requests"][1]["_id"].as_str().unwrap());

        server.request("GET", &host, "/three", &[], "").await;
        let newer = server
            .api(
                &token,
                "GET",
                &format!("/api/v2/requests?after={cursor}"),
                "",
            )
            .await
            .json();
        assert_eq!(paths(&newer), ["/three"]);
        assert_eq!(newer["pagination"]["has_more"], false);

        // Nothing newer than the newest: an empty page, and the cursor stays put
        let newest = newer["cursor"].as_str().unwrap();
        let empty = server
            .api(
                &token,
                "GET",
                &format!("/api/v2/requests?after={newest}"),
                "",
            )
            .await
            .json();
        assert_eq!(empty["requests"], json!([]));
        assert_eq!(empty["cursor"], newest);

        let only_dns = server
            .api(&token, "GET", "/api/v2/requests?type=dns", "")
            .await
            .json();
        assert_eq!(only_dns["requests"], json!([]));
        let only_http = server
            .api(&token, "GET", "/api/v2/requests?type=http&offset=1", "")
            .await
            .json();
        assert_eq!(paths(&only_http), ["/two", "/three"]);
        assert_eq!(only_http["pagination"]["total"], 3);

        let bad_type = server
            .api(&token, "GET", "/api/v2/requests?type=ftp", "")
            .await;
        assert_eq!(bad_type.status, 400);
        assert_eq!(bad_type.code(), "invalid_type");

        let unknown = server
            .api(&token, "GET", "/api/v2/requests?after=no-such-id", "")
            .await;
        assert_eq!(unknown.status, 404);
        assert_eq!(unknown.code(), "cursor_not_found");
    }

    #[tokio::test]
    async fn wait_holds_the_response_until_a_request_arrives() {
        let server = server().await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();
        let (addr, main_host) = (server.addr, server.main_host());

        let started = Instant::now();
        let poll = {
            let authorization = format!("Bearer {token}");
            tokio::spawn(async move {
                crate::tests::harness::http(
                    addr,
                    "GET",
                    &main_host,
                    "/api/v2/requests?wait=20",
                    &[("Authorization", authorization.as_str())],
                    "",
                )
                .await
            })
        };

        // Let the poll start waiting, then send the callback it is waiting for
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!poll.is_finished(), "answered before anything arrived");
        server.request("GET", &host, "/callback", &[], "").await;

        let reply = tokio::time::timeout(Duration::from_secs(5), poll)
            .await
            .expect("the long poll was not woken by the request")
            .unwrap();
        assert_eq!(paths(&reply.json()), ["/callback"]);
        assert!(started.elapsed() < Duration::from_secs(5));

        // Another session's traffic does not wake it
        let cursor = reply.json()["cursor"].as_str().unwrap().to_string();
        let other_host = server.subdomain_host(&get_random_subdomain());
        let started = Instant::now();
        let poll = {
            let authorization = format!("Bearer {token}");
            let (addr, main_host) = (server.addr, server.main_host());
            tokio::spawn(async move {
                crate::tests::harness::http(
                    addr,
                    "GET",
                    &main_host,
                    &format!("/api/v2/requests?after={cursor}&wait=1"),
                    &[("Authorization", authorization.as_str())],
                    "",
                )
                .await
            })
        };
        server.request("GET", &other_host, "/noise", &[], "").await;
        let timed_out = poll.await.unwrap().json();
        assert_eq!(timed_out["requests"], json!([]));
        assert!(started.elapsed() >= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn wait_is_capped() {
        let server = server().await;
        let token = generate_jwt(&get_random_subdomain()).unwrap();
        // Something is already there, so even an absurd wait answers at once
        server
            .request(
                "GET",
                &server.subdomain_host(&crate::utils::verify_jwt(&token).unwrap()),
                "/",
                &[],
                "",
            )
            .await;
        let started = Instant::now();
        let reply = server
            .api(&token, "GET", "/api/v2/requests?wait=99999", "")
            .await;
        assert_eq!(reply.status, 200);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    // --- Response files ----------------------------------------------------

    #[tokio::test]
    async fn one_file_can_be_set_as_text_and_is_served() {
        let server = server().await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();

        let put = server
            .api(
                &token,
                "PUT",
                "/api/v2/files/js/x.js",
                r#"{"body": "alert(1)"}"#,
            )
            .await;
        assert_eq!(put.status, 201, "{}", put.body);
        let put = put.json();
        assert_eq!(put["path"], "js/x.js");
        assert_eq!(
            put["url"],
            format!("{}/js/x.js", crate::http::routes_v2::public_origin(&host)).as_str()
        );

        let served = server.request("GET", &host, "/js/x.js", &[], "").await;
        assert_eq!(served.status, 200);
        assert_eq!(served.body, "alert(1)");
        assert_eq!(
            served.header("content-type"),
            Some("text/javascript; charset=utf-8")
        );

        // Replacing it with full control over status and headers
        let redirect = server
            .api(
                &token,
                "PUT",
                "/api/v2/files/js/x.js",
                r#"{"status_code": 302, "headers": [{"header": "Location", "value": "http://127.0.0.1/"}]}"#,
            )
            .await;
        assert_eq!(redirect.status, 200);
        let served = server.request("GET", &host, "/js/x.js", &[], "").await;
        assert_eq!(served.status, 302);
        assert_eq!(served.header("location"), Some("http://127.0.0.1/"));

        // Binary bodies go in `raw`
        let raw = server
            .api(&token, "PUT", "/api/v2/files/b.bin", r#"{"raw": "AAEC"}"#)
            .await;
        assert_eq!(raw.status, 201);

        for (body, code) in [
            (r#"{"body": "a", "raw": "YQ=="}"#, "invalid_body"),
            (r#"{"raw": "not base64!"}"#, "invalid_raw"),
            (r#"{"status_code": 42}"#, "invalid_status_code"),
        ] {
            let reply = server.api(&token, "PUT", "/api/v2/files/bad", body).await;
            assert_eq!(reply.status, 400, "{body}");
            assert_eq!(reply.code(), code, "{body}");
        }
        let traversal = server
            .api(&token, "PUT", "/api/v2/files/a/../b", r#"{"body": "x"}"#)
            .await;
        assert_eq!(traversal.status, 400);

        assert_eq!(
            server
                .api(&token, "DELETE", "/api/v2/files/js/x.js", "")
                .await
                .status,
            200
        );
        assert_eq!(
            server
                .api(&token, "DELETE", "/api/v2/files/js/x.js", "")
                .await
                .status,
            404
        );
    }

    // --- Discovery documents -----------------------------------------------

    #[tokio::test]
    async fn discovery_documents_are_served_on_the_main_domain() {
        let server = server().await;

        let llms = server.main("GET", "/llms.txt", &[]).await;
        assert_eq!(llms.status, 200);
        assert!(llms.body.starts_with("# requestrepo"));
        assert!(llms.body.contains("/docs/api.md"));
        assert_eq!(
            llms.header("content-signal"),
            Some("search=yes, ai-input=yes, ai-train=yes")
        );

        let guide = server.main("GET", "/docs/api.md", &[]).await;
        assert_eq!(
            guide.header("content-type"),
            Some("text/markdown; charset=utf-8")
        );
        assert!(guide.header("x-markdown-tokens").is_some());

        let skill = server.main("GET", "/skill.md", &[]).await;
        assert!(skill.body.starts_with("---\nname: requestrepo\n"));

        let openapi = server.main("GET", "/openapi.json", &[]).await.json();
        assert_eq!(openapi["openapi"], "3.1.0");

        let robots = server.main("GET", "/robots.txt", &[]).await;
        assert!(robots.body.contains("Content-Signal: search=yes"));
        assert!(robots.body.contains("Sitemap: "));

        let sitemap = server.main("GET", "/sitemap.xml", &[]).await;
        assert!(sitemap.body.contains("<loc>"));
    }

    #[tokio::test]
    async fn api_catalog_follows_rfc_9727() {
        let server = server().await;

        let catalog = server.main("GET", "/.well-known/api-catalog", &[]).await;
        assert_eq!(catalog.status, 200);
        assert_eq!(
            catalog.header("content-type"),
            Some("application/linkset+json; profile=\"https://www.rfc-editor.org/info/rfc9727\"")
        );
        let linkset = catalog.json();
        assert!(linkset["linkset"][0]["anchor"]
            .as_str()
            .unwrap()
            .ends_with("/api/v2"));

        // HEAD answers with the api-catalog Link relation and no body
        let head = server.main("HEAD", "/.well-known/api-catalog", &[]).await;
        assert_eq!(head.status, 200);
        assert!(head.body.is_empty());
        assert!(head.header("link").unwrap().contains("rel=\"api-catalog\""));
    }

    #[tokio::test]
    async fn markdown_is_negotiated_on_pages_people_open() {
        let server = server().await;

        let home = server
            .main("GET", "/", &[("Accept", "text/markdown, text/html;q=0.9")])
            .await;
        assert_eq!(home.status, 200);
        assert_eq!(
            home.header("content-type"),
            Some("text/markdown; charset=utf-8")
        );
        assert_eq!(home.header("vary"), Some("Accept"));
        assert!(home.body.starts_with("# requestrepo API guide"));

        let abuse = server
            .main("GET", "/abuse", &[("Accept", "text/markdown")])
            .await;
        assert!(abuse.body.starts_with("# Reporting abuse"));

        // A browser's Accept header gets the dashboard, never Markdown
        let browser = server
            .main(
                "GET",
                "/",
                &[(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )],
            )
            .await;
        assert_ne!(
            browser.header("content-type"),
            Some("text/markdown; charset=utf-8")
        );
    }

    #[tokio::test]
    async fn subdomains_keep_their_own_robots_txt_and_llms_txt() {
        let server = server().await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();

        server
            .api(
                &token,
                "PUT",
                "/api/v2/files/robots.txt",
                r#"{"body": "User-agent: *\nDisallow: /secret"}"#,
            )
            .await;
        let robots = server.request("GET", &host, "/robots.txt", &[], "").await;
        assert_eq!(robots.body, "User-agent: *\nDisallow: /secret");

        // A path the session never defined falls back to its own index.html, as always,
        // and is captured like any other request
        let llms = server.request("GET", &host, "/llms.txt", &[], "").await;
        assert!(!llms.body.contains("# requestrepo"));
        let markdown = server
            .request("GET", &host, "/", &[("Accept", "text/markdown")], "")
            .await;
        assert!(!markdown.body.contains("API guide"));
        assert_eq!(
            server
                .cache
                .llen(&format!("requests:{label}"))
                .await
                .unwrap(),
            3
        );
    }
}
