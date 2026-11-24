//! End-to-end tests for the subdomain blocklist.
//!
//! Each test starts the real router on a loopback port and talks to it over plain TCP, so
//! the HTTP middleware, the API and the WebSocket are exercised exactly as deployed.

#[cfg(test)]
mod tests {
    use crate::abuse::{AbuseList, Category, Source};
    use crate::cache::Cache;
    use crate::http::{create_router, AppState, StaticFiles};
    use crate::utils::config::CONFIG;
    use crate::utils::{generate_jwt, get_random_subdomain};
    use serde_json::{json, Value};
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::broadcast;

    const ADMIN: &str = "secret";
    const DESCRIPTION: &str = "Loader beacons here before dropping a miner";
    const JSON: &[(&str, &str)] = &[("Content-Type", "application/json")];

    fn report_body(subdomain: &str) -> String {
        json!({
            "subdomain": subdomain,
            "category": "malware",
            "description": DESCRIPTION,
            "evidence_url": "https://threatfox.abuse.ch/ioc/1982219/",
        })
        .to_string()
    }

    struct TestServer {
        addr: SocketAddr,
        cache: Arc<Cache>,
        abuse: Arc<AbuseList>,
        _data_dir: TempDir,
    }

    impl TestServer {
        async fn start(reporting_enabled: bool, admin_token: Option<&str>) -> Self {
            let data_dir = TempDir::new().unwrap();
            let abuse = Arc::new(
                AbuseList::load(
                    data_dir.path(),
                    reporting_enabled,
                    admin_token.map(str::to_string),
                )
                .await
                .unwrap(),
            );
            let cache = Arc::new(Cache::new());
            let (tx, _) = broadcast::channel(1024);
            let state = AppState {
                cache: cache.clone(),
                tx: Arc::new(tx),
                static_files: Arc::new(StaticFiles::load("./public").await),
                http_challenge_handler: None,
                abuse: abuse.clone(),
            };

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move {
                axum::serve(
                    listener,
                    create_router(state).into_make_service_with_connect_info::<SocketAddr>(),
                )
                .await
                .unwrap();
            });

            Self {
                addr,
                cache,
                abuse,
                _data_dir: data_dir,
            }
        }

        /// The main domain, where the dashboard and the API live.
        fn main_host(&self) -> String {
            CONFIG.server_domain.clone()
        }

        /// The host a subdomain's own traffic arrives with.
        fn subdomain_host(&self, subdomain: &str) -> String {
            format!("{subdomain}.{}", CONFIG.server_domain)
        }

        async fn request(
            &self,
            method: &str,
            host: &str,
            target: &str,
            headers: &[(&str, &str)],
            body: &str,
        ) -> Reply {
            http(self.addr, method, host, target, headers, body).await
        }

        async fn report(&self, subdomain: &str) -> Reply {
            self.request(
                "POST",
                &self.main_host(),
                "/api/v2/abuse/reports",
                JSON,
                &report_body(subdomain),
            )
            .await
        }

        async fn admin(&self, method: &str, target: &str) -> Reply {
            let authorization = format!("Bearer {ADMIN}");
            self.request(
                method,
                &self.main_host(),
                target,
                &[("Authorization", authorization.as_str())],
                "",
            )
            .await
        }
    }

    /// A request to the API on the main domain, authenticated with `authorization`.
    /// A non-empty `body` is sent as JSON.
    async fn api(
        server: &TestServer,
        authorization: &str,
        method: &str,
        target: &str,
        body: &str,
    ) -> Reply {
        let mut headers = vec![("Authorization", authorization)];
        if !body.is_empty() {
            headers.push(("Content-Type", "application/json"));
        }
        server
            .request(method, &server.main_host(), target, &headers, body)
            .await
    }

    struct Reply {
        status: u16,
        headers: Vec<(String, String)>,
        body: String,
    }

    impl Reply {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }

        fn json(&self) -> Value {
            serde_json::from_str(&self.body)
                .unwrap_or_else(|e| panic!("response is not JSON ({e}): {}", self.body))
        }

        fn code(&self) -> String {
            self.json()["code"].as_str().unwrap().to_string()
        }
    }

    /// Sends one HTTP/1.1 request with `Connection: close` and reads the whole response.
    async fn http(
        addr: SocketAddr,
        method: &str,
        host: &str,
        target: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Reply {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let mut request =
            format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
        if !body.is_empty() {
            request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        // The server may close before a body it has already declined arrives
        let _ = stream.write_all(body.as_bytes()).await;

        let mut raw = Vec::new();
        // A failed read says nothing on its own: say which request it belonged to
        if let Err(e) = stream.read_to_end(&mut raw).await {
            panic!(
                "reading the response to {method} {target} failed after {} bytes: {e}",
                raw.len()
            );
        }
        parse_response(&raw)
    }

    fn parse_response(raw: &[u8]) -> Reply {
        let split = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap_or_else(|| {
                panic!(
                    "response has no header terminator: {}",
                    String::from_utf8_lossy(raw)
                )
            });
        let head = String::from_utf8_lossy(&raw[..split]).to_string();
        let body = String::from_utf8_lossy(&raw[split + 4..]).to_string();

        let mut lines = head.split("\r\n");
        let status_line = lines.next().unwrap();
        let status: u16 = status_line.split(' ').nth(1).unwrap().parse().unwrap();
        let headers = lines
            .filter_map(|line| line.split_once(": "))
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        Reply {
            status,
            headers,
            body,
        }
    }

    // --- Reporting ---------------------------------------------------------

    #[tokio::test]
    async fn report_blocks_and_repeating_it_changes_nothing_visible() {
        let server = TestServer::start(true, None).await;
        let label = get_random_subdomain();

        let first = server.report(&label).await;
        assert_eq!(first.status, 201);
        assert_eq!(first.json()["status"], "blocked");
        assert_eq!(first.json()["subdomain"], label);
        assert_eq!(first.json()["persisted"], true);
        assert!(server.abuse.is_blocked(&label));

        // Saved where an operator can find it
        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(server._data_dir.path().join("abuse.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["version"], 1);
        assert_eq!(saved["entries"][0]["subdomain"], label);

        let second = server.report(&label).await;
        assert_eq!(second.status, 200);
        assert_eq!(second.json()["status"], "already_blocked");

        let entry = server.abuse.entry(&label).unwrap();
        assert_eq!(entry.source, Source::Report);
        assert_eq!(entry.category, Category::Malware);
        assert_eq!(entry.report_count, 2);
        assert_eq!(entry.reports.len(), 2);
        assert_eq!(entry.reports[0].description, DESCRIPTION);
        // The exact reporter address is not kept
        assert_eq!(entry.reports[0].network, "127.0.0.0/24");
    }

    #[tokio::test]
    async fn report_accepts_a_url_and_rejects_bad_input_without_spending_the_budget() {
        let server = TestServer::start(true, None).await;
        let host = server.subdomain_host(&get_random_subdomain());
        let api = "/api/v2/abuse/reports";

        let as_url = json!({
            "subdomain": format!("http://{host}/bottom-xe.png"),
            "category": "malware",
            "description": DESCRIPTION,
        })
        .to_string();
        assert_eq!(
            server
                .request("POST", &server.main_host(), api, JSON, &as_url)
                .await
                .status,
            201
        );

        // Each of these is refused, and none of them uses up one of the three attempts
        let bad_input = [
            json!({ "subdomain": "not-a-label", "category": "malware", "description": DESCRIPTION }),
            json!({ "subdomain": "abcdefgh.example.org", "category": "malware", "description": DESCRIPTION }),
            json!({ "subdomain": "abcdefgh", "category": "malware", "description": "   " }),
            json!({ "subdomain": "abcdefgh", "category": "malware", "description": "x".repeat(1001) }),
            json!({ "subdomain": "abcdefgh", "category": "malware", "description": DESCRIPTION, "evidence_url": "javascript:alert(1)" }),
            json!({ "subdomain": "abcdefgh", "category": "malware", "description": DESCRIPTION, "contact": "c".repeat(201) }),
        ];
        for body in bad_input {
            let reply = server
                .request("POST", &server.main_host(), api, JSON, &body.to_string())
                .await;
            assert_eq!(reply.status, 400, "{body}: {}", reply.body);
        }

        // Two more reports fit in the budget of three
        for _ in 0..2 {
            assert_eq!(server.report(&get_random_subdomain()).await.status, 201);
        }

        let limited = server.report(&get_random_subdomain()).await;
        assert_eq!(limited.status, 429);
        assert_eq!(limited.code(), "rate_limited");
        let retry_after: u64 = limited.header("retry-after").unwrap().parse().unwrap();
        assert!((3595..=3600).contains(&retry_after), "{retry_after}");
        assert_eq!(limited.json()["retry_after"], retry_after);
        assert_eq!(limited.json()["persisted"], Value::Null);

        // Still enforced a moment later: nothing was blocked by the refused request
        assert_eq!(server.abuse.blocked_count(), 3);
    }

    #[tokio::test]
    async fn report_demands_a_small_json_body() {
        let server = TestServer::start(true, None).await;
        let label = get_random_subdomain();
        let api = "/api/v2/abuse/reports";
        let limit = std::time::Duration::from_secs(5);

        // Both are refused on the headers alone, so the body is never sent. If the server
        // waited for a body it was going to refuse, these would time out.
        let not_json = tokio::time::timeout(
            limit,
            server.request(
                "POST",
                &server.main_host(),
                api,
                &[("Content-Type", "text/plain"), ("Content-Length", "400")],
                "",
            ),
        )
        .await
        .expect("waited for a body that is not JSON");
        assert_eq!(not_json.status, 415);

        // The limit is enforced while reading, not from the Content-Length header: one
        // byte over it is read in full and then refused
        let oversized = "x".repeat(16 * 1024 + 1);
        let too_big = tokio::time::timeout(
            limit,
            server.request("POST", &server.main_host(), api, JSON, &oversized),
        )
        .await
        .expect("did not answer a body over its limit");
        assert_eq!(too_big.status, 413);

        // JSON that parses but is not the right shape is refused too
        let wrong_shape = server
            .request("POST", &server.main_host(), api, JSON, "[1, 2, 3]")
            .await;
        assert_eq!(wrong_shape.status, 422);

        assert!(!server.abuse.is_blocked(&label));
    }

    #[tokio::test]
    async fn concurrent_reports_cannot_exceed_the_rate_limit() {
        let server = TestServer::start(true, None).await;

        let mut tasks = Vec::new();
        for _ in 0..24 {
            let body = report_body(&get_random_subdomain());
            let host = server.main_host();
            tasks.push(tokio::spawn(async move {
                http(
                    server.addr,
                    "POST",
                    &host,
                    "/api/v2/abuse/reports",
                    JSON,
                    &body,
                )
                .await
                .status
            }));
        }
        let mut statuses = Vec::new();
        for task in tasks {
            statuses.push(task.await.unwrap());
        }

        assert_eq!(statuses.iter().filter(|status| **status == 201).count(), 3);
        assert_eq!(statuses.iter().filter(|status| **status == 429).count(), 21);
        assert_eq!(server.abuse.blocked_count(), 3);
    }

    #[tokio::test]
    async fn report_endpoint_is_not_advertised_when_disabled() {
        let server = TestServer::start(false, None).await;

        let reply = server.report(&get_random_subdomain()).await;
        assert_eq!(reply.status, 404);
        assert_eq!(reply.code(), "reporting_disabled");
    }

    #[tokio::test]
    async fn report_endpoint_answers_without_cors_headers() {
        let server = TestServer::start(true, None).await;
        let reply = server.report(&get_random_subdomain()).await;

        assert_eq!(reply.status, 201);
        // Other API routes send these; the public report route must not, or any site
        // could make its visitors file reports from their own addresses
        assert_eq!(reply.header("access-control-allow-origin"), None);
    }

    // --- Admin -------------------------------------------------------------

    #[tokio::test]
    async fn admin_endpoints_need_a_configured_token() {
        let unconfigured = TestServer::start(true, None).await;
        let reply = unconfigured
            .request(
                "GET",
                &unconfigured.main_host(),
                "/api/v2/abuse/admin/entries",
                &[("Authorization", "Bearer anything")],
                "",
            )
            .await;
        assert_eq!(reply.status, 403);
        assert_eq!(reply.code(), "admin_not_configured");

        let configured = TestServer::start(true, Some(ADMIN)).await;
        let entries = "/api/v2/abuse/admin/entries";
        let host = configured.main_host();
        let (no_token, wrong_token) = tokio::join!(
            configured.request("GET", &host, entries, &[], ""),
            configured.request(
                "GET",
                &host,
                entries,
                &[("Authorization", "Bearer hunter2")],
                "",
            ),
        );
        for reply in [&no_token, &wrong_token] {
            // Missing and wrong are indistinguishable, and neither reveals the real one
            assert_eq!(reply.status, 401);
            assert_eq!(reply.code(), "invalid_admin_token");
        }

        let by_cookie = configured
            .request(
                "GET",
                &configured.main_host(),
                entries,
                &[("Cookie", "admin_token=secret")],
                "",
            )
            .await;
        assert_eq!(by_cookie.status, 200);
        assert_eq!(by_cookie.json()["entries"], json!([]));
    }

    #[tokio::test]
    async fn admin_blocks_lifts_and_lists() {
        let server = TestServer::start(true, Some(ADMIN)).await;
        let label = get_random_subdomain();
        let entries = "/api/v2/abuse/admin/entries";

        let block = json!({
            "admin_token": ADMIN,
            "category": "phishing",
            "note": "Seen in a phishing kit",
        })
        .to_string();
        let created = server
            .request(
                "POST",
                &server.main_host(),
                &format!("{entries}/{label}/block"),
                JSON,
                &block,
            )
            .await;
        assert_eq!(created.status, 201);
        assert!(server.abuse.is_blocked(&label));

        // Blocking again, this time with nothing but the token: no body is needed
        let again = server
            .admin("POST", &format!("{entries}/{label}/block"))
            .await;
        assert_eq!(again.status, 200);
        assert_eq!(again.json()["status"], "already_blocked");

        // A body that cannot be parsed is refused, never treated as absent
        let malformed = server
            .request(
                "POST",
                &server.main_host(),
                &format!("{entries}/{label}/block"),
                JSON,
                "{not json",
            )
            .await;
        assert_eq!(malformed.status, 400);
        assert_eq!(malformed.code(), "invalid_body");

        // A body that parses but has the wrong shape is refused as well
        let wrong_shape = server
            .request(
                "POST",
                &server.main_host(),
                &format!("{entries}/{label}/block"),
                JSON,
                r#"{"category": "not-a-category"}"#,
            )
            .await;
        assert_eq!(wrong_shape.status, 400);

        let listed = server.admin("GET", entries).await;
        assert_eq!(listed.json()["total"], 1);
        assert_eq!(listed.json()["entries"][0]["subdomain"], label);
        assert_eq!(listed.json()["entries"][0]["source"], "admin");
        assert_eq!(listed.json()["entries"][0]["category"], "phishing");
        // The list leaves the reports out; the detail has everything
        assert_eq!(listed.json()["entries"][0]["reports"], Value::Null);

        let detail = server.admin("GET", &format!("{entries}/{label}")).await;
        assert_eq!(detail.json()["admin_note"], "Seen in a phishing kit");

        // Lifted: gone from the list, and the lift is saved
        let lifted = server.admin("DELETE", &format!("{entries}/{label}")).await;
        assert_eq!(lifted.status, 200);
        assert!(!server.abuse.is_blocked(&label));
        assert_eq!(server.admin("GET", entries).await.json()["total"], 0);
        assert_eq!(
            server
                .admin("DELETE", &format!("{entries}/{label}"))
                .await
                .status,
            404
        );
        assert!(
            !AbuseList::load(server._data_dir.path(), true, Some(ADMIN.into()))
                .await
                .unwrap()
                .is_blocked(&label)
        );
    }

    // --- Enforcement -------------------------------------------------------

    #[tokio::test]
    async fn blocked_subdomain_traffic_is_refused_and_never_logged() {
        let server = TestServer::start(true, Some(ADMIN)).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let request_log = format!("requests:{label}");

        // Before: a request is answered and logged
        let before = server.request("GET", &host, "/", &[], "").await;
        assert_eq!(before.status, 200);
        assert_eq!(server.cache.llen(&request_log).await.unwrap(), 1);

        assert_eq!(server.report(&label).await.status, 201);

        let refused = server.request("GET", &host, "/beacon", &[], "").await;
        assert_eq!(refused.status, 410);
        assert!(refused.body.contains("disabled for abuse"));
        assert_eq!(refused.header("cache-control"), Some("no-store"));
        assert_eq!(refused.header("connection"), Some("close"));

        // The body is never awaited: a request that announces a megabyte and sends none
        // of it is answered straight away
        let announced = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            server.request(
                "POST",
                &host,
                "/upload",
                &[
                    ("Content-Type", "application/octet-stream"),
                    ("Content-Length", "1048576"),
                ],
                "",
            ),
        )
        .await
        .expect("the server waited for the body of a blocked subdomain");
        assert_eq!(announced.status, 410);

        // Names below the blocked one are refused too
        let nested = server
            .request(
                "GET",
                &server.subdomain_host(&format!("data.{label}")),
                "/",
                &[],
                "",
            )
            .await;
        assert_eq!(nested.status, 410);

        // The dashboard host is untouched
        let dashboard = server
            .request("GET", &server.main_host(), "/", &[], "")
            .await;
        assert_ne!(dashboard.status, 410);

        assert_eq!(server.cache.llen(&request_log).await.unwrap(), 1);

        // Lifting restores the subdomain
        server
            .admin("DELETE", &format!("/api/v2/abuse/admin/entries/{label}"))
            .await;
        let restored = server.request("GET", &host, "/", &[], "").await;
        assert_eq!(restored.status, 200);
    }

    #[tokio::test]
    async fn owner_loses_access_while_blocked_and_recovers_after_a_lift() {
        let server = TestServer::start(true, Some(ADMIN)).await;
        let label = get_random_subdomain();
        let other = get_random_subdomain();
        let token = generate_jwt(&label).unwrap();
        let bearer = format!("Bearer {token}");

        assert_eq!(
            api(&server, &bearer, "GET", "/api/v2/files", "")
                .await
                .status,
            200
        );

        // A shared request link made before the block
        server
            .cache
            .rpush(
                &format!("requests:{label}"),
                &json!({ "_id": "req-1", "type": "http", "raw": "", "uid": label }).to_string(),
            )
            .await
            .unwrap();
        // No body on purpose. This endpoint reads none, and a body that reaches the server
        // after it has closed the connection turns the close into a reset, which would make
        // this test fail at random. Browsers keep connections alive and are not affected.
        let shared = api(&server, &bearer, "POST", "/api/v2/requests/req-1/share", "").await;
        assert_eq!(shared.status, 201);
        let share_target = format!(
            "/api/v2/requests/shared/{}",
            shared.json()["share_token"].as_str().unwrap()
        );
        assert_eq!(
            api(&server, &bearer, "GET", &share_target, "").await.status,
            200
        );

        assert_eq!(server.report(&label).await.status, 201);

        // Bodies are valid, so that each request is refused by the block and not by a
        // body parser that runs first
        for (method, target, body) in [
            ("GET", "/api/v2/files", ""),
            ("GET", "/api/v2/dns", ""),
            ("PUT", "/api/v2/dns", r#"{"records":[]}"#),
            ("PUT", "/api/v2/files", "{}"),
            ("GET", "/api/v2/requests", ""),
            ("DELETE", "/api/v2/requests", ""),
            ("GET", "/api/v2/requests/req-1", ""),
            ("DELETE", "/api/v2/requests/req-1", ""),
        ] {
            let reply = api(&server, &bearer, method, target, body).await;
            assert_eq!(reply.status, 403, "{method} {target}: {}", reply.body);
            // "Blocked", deliberately not "invalid token": the dashboard drops sessions
            // whose token it considers invalid
            assert_eq!(reply.code(), "subdomain_blacklisted");
            assert_eq!(reply.json()["category"], "malware");
        }
        let link = api(&server, &bearer, "GET", &share_target, "").await;
        assert_eq!(link.status, 403);
        assert_eq!(link.code(), "subdomain_blacklisted");

        // Other subdomains are unaffected
        let other_authorization = format!("Bearer {}", generate_jwt(&other).unwrap());
        let untouched = server
            .request(
                "GET",
                &server.main_host(),
                "/api/v2/files",
                &[("Authorization", other_authorization.as_str())],
                "",
            )
            .await;
        assert_eq!(untouched.status, 200);

        // The same token works again once the block is lifted
        server
            .admin("DELETE", &format!("/api/v2/abuse/admin/entries/{label}"))
            .await;
        assert_eq!(
            api(&server, &bearer, "GET", "/api/v2/files", "")
                .await
                .status,
            200
        );
    }

    #[tokio::test]
    async fn captured_requests_stay_readable_by_the_admin_until_purged() {
        let server = TestServer::start(true, Some(ADMIN)).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let entries = format!("/api/v2/abuse/admin/entries/{label}");

        server.request("GET", &host, "/first", &[], "").await;
        server.request("GET", &host, "/second", &[], "").await;
        assert_eq!(server.report(&label).await.status, 201);

        let captured = server.admin("GET", &format!("{entries}/requests")).await;
        assert_eq!(captured.status, 200);
        let captured = captured.json();
        let paths: Vec<&str> = captured["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|request| request["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, ["/first", "/second"]);
        assert_eq!(captured["pagination"]["total"], 2);

        let purged = server.admin("DELETE", &format!("{entries}/data")).await;
        assert_eq!(purged.status, 200);
        assert_eq!(
            server
                .cache
                .llen(&format!("requests:{label}"))
                .await
                .unwrap(),
            0
        );
        let after = server.admin("GET", &format!("{entries}/requests")).await;
        assert_eq!(after.json()["requests"], json!([]));

        // Purging does not lift the block
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 410);
    }

    #[tokio::test]
    async fn purge_removes_files_and_every_dns_record() {
        let server = TestServer::start(true, Some(ADMIN)).await;
        let label = get_random_subdomain();
        let token = generate_jwt(&label).unwrap();
        let bearer = format!("Bearer {token}");

        let dns = json!({ "records": [
            { "domain": "www", "type": "A", "value": "6.6.6.6" },
            { "domain": "*", "type": "TXT", "value": "stolen-data" },
        ]})
        .to_string();
        assert_eq!(
            server
                .request("PUT", &server.main_host(), "/api/v2/dns", JSON, &dns)
                .await
                .status,
            401,
            "the request needs the owner's token"
        );
        server
            .request(
                "PUT",
                &server.main_host(),
                "/api/v2/dns",
                &[
                    ("Authorization", bearer.as_str()),
                    ("Content-Type", "application/json"),
                ],
                &dns,
            )
            .await;
        // Fetching the files creates the default file tree, as it does for a new session
        server
            .request(
                "GET",
                &server.main_host(),
                "/api/v2/files",
                &[("Authorization", bearer.as_str())],
                "",
            )
            .await;
        assert!(server
            .cache
            .get(&format!("files:{label}"))
            .await
            .unwrap()
            .is_some());
        assert!(server
            .cache
            .get(&format!("dns:A:www.{label}.{}.", CONFIG.server_domain))
            .await
            .unwrap()
            .is_some());

        assert_eq!(server.report(&label).await.status, 201);
        let purged = server
            .admin(
                "DELETE",
                &format!("/api/v2/abuse/admin/entries/{label}/data"),
            )
            .await;
        assert_eq!(purged.status, 200);

        for key in [
            format!("files:{label}"),
            format!("dns:{label}"),
            format!("dns:A:www.{label}.{}.", CONFIG.server_domain),
            format!("dns:TXT:*.{label}.{}.", CONFIG.server_domain),
        ] {
            assert_eq!(server.cache.get(&key).await.unwrap(), None, "{key}");
        }
    }

    #[tokio::test]
    async fn health_reports_the_blocklist() {
        let server = TestServer::start(true, None).await;
        let before = server
            .request("GET", &server.main_host(), "/health", &[], "")
            .await
            .json();
        assert_eq!(before["abuse"]["blocked_subdomains"], 0);
        assert_eq!(before["abuse"]["persist_failures"], 0);

        server.report(&get_random_subdomain()).await;
        let after = server
            .request("GET", &server.main_host(), "/health", &[], "")
            .await
            .json();
        assert_eq!(after["abuse"]["blocked_subdomains"], 1);
    }

    // --- WebSocket ---------------------------------------------------------

    /// Opens a WebSocket and returns the upgraded stream.
    async fn websocket(addr: SocketAddr, host: &str) -> TcpStream {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let request = format!(
            "GET /api/v2/ws HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.unwrap();

        // Read the response head past the upgrade; the socket then carries frames
        let mut head = Vec::new();
        let mut byte = [0u8];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).await.unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head).to_string();
        assert!(head.starts_with("HTTP/1.1 101"), "{head}");
        stream
    }

    /// Sends one masked text frame, as a browser would.
    async fn ws_send(stream: &mut TcpStream, text: &str) {
        assert!(text.len() <= u16::MAX as usize, "test frames are short");
        const MASK: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
        let mut frame = vec![0x81];
        match text.len() {
            n if n < 126 => frame.push(0x80 | n as u8),
            n => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(n as u16).to_be_bytes());
            }
        }
        frame.extend_from_slice(&MASK);
        frame.extend(
            text.bytes()
                .zip(MASK.iter().cycle())
                .map(|(byte, mask)| byte ^ mask),
        );
        stream.write_all(&frame).await.unwrap();
    }

    /// Reads one text frame and returns its payload.
    async fn ws_recv(stream: &mut TcpStream) -> Value {
        let mut head = [0u8; 2];
        stream.read_exact(&mut head).await.unwrap();
        assert_eq!(head[0] & 0x0f, 0x1, "expected a text frame");
        assert_eq!(head[1] & 0x80, 0, "server frames are not masked");
        let length = match head[1] & 0x7f {
            n if n < 126 => n as usize,
            126 => {
                let mut extended = [0u8; 2];
                stream.read_exact(&mut extended).await.unwrap();
                u16::from_be_bytes(extended) as usize
            }
            _ => panic!("test frames are short"),
        };
        let mut payload = vec![0u8; length];
        stream.read_exact(&mut payload).await.unwrap();
        serde_json::from_slice(&payload).expect("frame is JSON")
    }

    #[tokio::test]
    async fn websocket_refuses_a_blocked_token_with_its_own_code() {
        let server = TestServer::start(true, None).await;
        let label = get_random_subdomain();
        let other = get_random_subdomain();
        assert_eq!(server.report(&label).await.status, 201);

        let mut socket = websocket(server.addr, &server.main_host()).await;

        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": generate_jwt(&label).unwrap() }).to_string(),
        )
        .await;
        let blocked = ws_recv(&mut socket).await;
        assert_eq!(blocked["cmd"], "error");
        // Must not be `invalid_token`: the dashboard deletes sessions with an invalid token
        assert_eq!(blocked["code"], "subdomain_blacklisted");
        assert_eq!(blocked["subdomain"], label);
        assert_eq!(blocked["category"], "malware");

        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": generate_jwt(&other).unwrap() }).to_string(),
        )
        .await;
        assert_eq!(ws_recv(&mut socket).await["cmd"], "connected");

        // Tokens that really are invalid are still reported that way
        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": "garbage" }).to_string(),
        )
        .await;
        assert_eq!(ws_recv(&mut socket).await["code"], "invalid_token");
    }

    #[tokio::test]
    async fn connected_dashboard_is_told_when_its_session_is_blocked() {
        let server = TestServer::start(true, None).await;
        let label = get_random_subdomain();

        let mut socket = websocket(server.addr, &server.main_host()).await;
        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": generate_jwt(&label).unwrap() }).to_string(),
        )
        .await;
        assert_eq!(ws_recv(&mut socket).await["cmd"], "connected");

        assert_eq!(server.report(&label).await.status, 201);

        let notice = ws_recv(&mut socket).await;
        assert_eq!(notice["cmd"], "blacklisted");
        assert_eq!(notice["subdomain"], label);
        // The reason travels with the notice, so the dashboard can explain the block at once
        assert_eq!(notice["category"], "malware");
    }

    // --- DNS ---------------------------------------------------------------

    async fn dns_query(
        server: &TestServer,
        name: &str,
    ) -> (hickory_proto::op::ResponseCode, usize) {
        use hickory_proto::op::{Message, MessageType, OpCode, Query};
        use hickory_proto::rr::{Name, RecordType};
        use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};
        use std::str::FromStr;

        let mut query = Message::new(0x1234, MessageType::Query, OpCode::Query);
        let mut question = Query::new();
        question.set_name(Name::from_str(name).unwrap());
        question.set_query_type(RecordType::A);
        query.add_query(question);

        let (tx, _) = broadcast::channel(16);
        let bytes = crate::dns::process_dns_query(
            &query.to_bytes().unwrap(),
            "127.0.0.1:5353".parse().unwrap(),
            server.cache.clone(),
            Arc::new(tx),
            server.abuse.clone(),
            false,
        )
        .await
        .unwrap();
        let response = Message::from_bytes(&bytes).unwrap();
        (response.metadata.response_code, response.answers.len())
    }

    #[tokio::test]
    async fn dns_treats_a_blocked_subdomain_as_nonexistent() {
        use hickory_proto::op::ResponseCode;

        let server = TestServer::start(true, None).await;
        let label = get_random_subdomain();
        let name = format!("{}.{}.", label, CONFIG.server_domain);
        let nested = format!("data.{}.", name.trim_end_matches('.'));

        assert_eq!(
            dns_query(&server, &name).await,
            (ResponseCode::NoError, 1),
            "answers with the server address before the block"
        );
        let logged_before_block = server
            .cache
            .llen(&format!("requests:{label}"))
            .await
            .unwrap();
        assert_eq!(logged_before_block, 1);

        assert_eq!(server.report(&label).await.status, 201);

        for blocked in [&name, &nested] {
            let (code, answers) = dns_query(&server, blocked).await;
            assert_eq!(code, ResponseCode::NXDomain, "{blocked}");
            assert_eq!(answers, 0, "{blocked}");
        }
        // Nothing about the blocked names was logged
        assert_eq!(
            server
                .cache
                .llen(&format!("requests:{label}"))
                .await
                .unwrap(),
            logged_before_block
        );

        let other = get_random_subdomain();
        assert_eq!(
            dns_query(&server, &format!("{other}.{}.", CONFIG.server_domain)).await,
            (ResponseCode::NoError, 1)
        );
    }
}
