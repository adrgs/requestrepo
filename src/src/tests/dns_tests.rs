#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    // Note: Integration tests for DNS would require running a DNS server
    // and making actual DNS queries. These tests verify the cache integration.

    #[tokio::test]
    async fn test_dns_cache_record_storage() {
        let cache = Arc::new(Cache::new());
        let (_tx, _) = broadcast::channel::<crate::models::CacheMessage>(1024);

        // Store a DNS record in the cache
        let key = "dns:A:test.abcdefgh.example.com.";
        let value = "1.2.3.4";

        cache.set(key, value).await.unwrap();

        // Retrieve the record
        let result = cache.get(key).await.unwrap();
        assert_eq!(result, Some(value.to_string()));
    }

    #[tokio::test]
    async fn test_dns_cache_multiple_record_types() {
        let cache = Arc::new(Cache::new());

        // Store different record types
        cache
            .set("dns:A:test.example.com.", "1.2.3.4")
            .await
            .unwrap();
        cache
            .set("dns:AAAA:test.example.com.", "::1")
            .await
            .unwrap();
        cache
            .set("dns:CNAME:test.example.com.", "other.example.com.")
            .await
            .unwrap();
        cache
            .set("dns:TXT:test.example.com.", "v=spf1 include:example.com")
            .await
            .unwrap();

        // Verify all records can be retrieved
        assert_eq!(
            cache.get("dns:A:test.example.com.").await.unwrap(),
            Some("1.2.3.4".to_string())
        );
        assert_eq!(
            cache.get("dns:AAAA:test.example.com.").await.unwrap(),
            Some("::1".to_string())
        );
        assert_eq!(
            cache.get("dns:CNAME:test.example.com.").await.unwrap(),
            Some("other.example.com.".to_string())
        );
        assert_eq!(
            cache.get("dns:TXT:test.example.com.").await.unwrap(),
            Some("v=spf1 include:example.com".to_string())
        );
    }

    #[tokio::test]
    async fn test_dns_subdomain_record_storage() {
        let cache = Arc::new(Cache::new());

        // Store subdomain DNS configuration
        let subdomain = "testsubdomain";
        let records_json =
            r#"[{"domain":"test.testsubdomain.example.com.","type":"A","value":"1.2.3.4"}]"#;

        cache
            .set(&format!("dns:{subdomain}"), records_json)
            .await
            .unwrap();

        // Verify retrieval
        let result = cache.get(&format!("dns:{subdomain}")).await.unwrap();
        assert_eq!(result, Some(records_json.to_string()));
    }

    #[tokio::test]
    async fn test_dns_record_deletion() {
        let cache = Arc::new(Cache::new());

        let key = "dns:A:test.example.com.";
        cache.set(key, "1.2.3.4").await.unwrap();

        // Delete the record
        let deleted = cache.delete(key).await.unwrap();
        assert!(deleted);

        // Verify it's gone
        let result = cache.get(key).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_dns_keys_pattern() {
        let cache = Arc::new(Cache::new());

        // Store multiple DNS records
        cache.set("dns:A:a.example.com.", "1.2.3.4").await.unwrap();
        cache.set("dns:A:b.example.com.", "5.6.7.8").await.unwrap();
        cache.set("dns:AAAA:c.example.com.", "::1").await.unwrap();

        // Find all A records
        let a_keys = cache.keys("dns:A:*").await.unwrap();
        assert_eq!(a_keys.len(), 2);

        // Find all dns keys
        let all_dns_keys = cache.keys("dns:*").await.unwrap();
        assert_eq!(all_dns_keys.len(), 3);
    }

    // --- Transport / response-code behavior -------------------------------

    use std::net::SocketAddr;
    use std::str::FromStr;
    use trust_dns_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
    use trust_dns_proto::rr::{Name, RecordType};
    use trust_dns_proto::serialize::binary::{BinDecodable, BinEncodable};

    fn build_query(name: &str, rtype: RecordType) -> Vec<u8> {
        let mut msg = Message::new();
        msg.set_id(0x1234);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(true);
        let mut q = Query::new();
        q.set_name(Name::from_str(name).unwrap());
        q.set_query_type(rtype);
        msg.add_query(q);
        msg.to_bytes().unwrap()
    }

    fn test_addr() -> SocketAddr {
        "127.0.0.1:5353".parse().unwrap()
    }

    /// An unhandled query type on our own zone must be NODATA (NOERROR, no
    /// answers) with an SOA in the authority section — NOT NXDOMAIN.
    #[tokio::test]
    async fn test_unhandled_type_is_nodata_not_nxdomain() {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel::<crate::models::CacheMessage>(1024);

        // DS query for the apex (localhost is the default test domain)
        let query = build_query("localhost.", RecordType::DS);
        let bytes = crate::dns::process_dns_query(&query, test_addr(), cache, Arc::new(tx), false)
            .await
            .unwrap();

        let resp = Message::from_bytes(&bytes).unwrap();
        assert_eq!(resp.response_code(), ResponseCode::NoError);
        assert_eq!(resp.answers().len(), 0);
        // SOA present in the authority section for negative caching
        assert_eq!(resp.name_server_count(), 1);
    }

    /// SOA and NS queries for the apex must be answered authoritatively.
    #[tokio::test]
    async fn test_soa_and_ns_answered() {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel::<crate::models::CacheMessage>(1024);

        for rtype in [RecordType::SOA, RecordType::NS] {
            let query = build_query("localhost.", rtype);
            let bytes = crate::dns::process_dns_query(
                &query,
                test_addr(),
                cache.clone(),
                Arc::new(tx.clone()),
                false,
            )
            .await
            .unwrap();
            let resp = Message::from_bytes(&bytes).unwrap();
            assert_eq!(resp.response_code(), ResponseCode::NoError, "{rtype:?}");
            assert_eq!(resp.answers().len(), 1, "{rtype:?} should have 1 answer");
        }
    }

    /// End-to-end DNS-over-TCP: a length-prefixed query yields a length-prefixed,
    /// well-formed, non-truncated response over a real TCP connection.
    #[tokio::test]
    async fn test_dns_over_tcp_roundtrip() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::{TcpListener, TcpStream};

        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel::<crate::models::CacheMessage>(1024);
        let rate_limiter = Arc::new(crate::dns::DnsRateLimiter::new(1000));

        // Bind an ephemeral port and serve exactly one connection
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let tx2 = Arc::new(tx);
        tokio::spawn(async move {
            let (stream, peer) = listener.accept().await.unwrap();
            crate::dns::handle_tcp_connection(stream, peer, cache, tx2, rate_limiter)
                .await
                .ok();
        });

        // Client: send a length-prefixed A query
        let mut client = TcpStream::connect(addr).await.unwrap();
        let query = build_query("test.localhost.", RecordType::A);
        let len = u16::try_from(query.len()).unwrap();
        client.write_all(&len.to_be_bytes()).await.unwrap();
        client.write_all(&query).await.unwrap();
        client.flush().await.unwrap();

        // Read the 2-byte length prefix then the message
        let mut len_buf = [0u8; 2];
        client.read_exact(&mut len_buf).await.unwrap();
        let resp_len = u16::from_be_bytes(len_buf) as usize;
        assert!(resp_len > 0);
        let mut resp_bytes = vec![0u8; resp_len];
        client.read_exact(&mut resp_bytes).await.unwrap();

        let resp = Message::from_bytes(&resp_bytes).unwrap();
        assert_eq!(resp.id(), 0x1234);
        assert_eq!(resp.message_type(), MessageType::Response);
        assert!(!resp.truncated(), "TCP responses must never be truncated");
        assert_eq!(resp.answers().len(), 1); // default A -> server_ip
    }
}
