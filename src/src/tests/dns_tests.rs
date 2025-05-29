#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::dns::DnsRequestHandler;
    use crate::utils::config::CONFIG;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use tokio::net::UdpSocket;
    use tokio::sync::broadcast;
    use trust_dns_proto::op::{Message, MessageType, Query};
    use rand;
    use trust_dns_proto::rr::{Name, RData, RecordType};
    use trust_dns_proto::udp::UdpClientStream;
    use trust_dns_proto::xfer::{DnsExchange, DnsRequest};
    use trust_dns_proto::DnsHandle;
    use trust_dns_proto::xfer::DnsRequestOptions;
    use trust_dns_proto::TokioTime;
    use futures_util::StreamExt;
    use trust_dns_server::ServerFuture;

    #[tokio::test]
    async fn test_dns_a_record() {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        
        // Use port 5353 for this test
        let port = 5353;
        
        // Create a domain name that matches the format expected by the DNS handler
        let subdomain = CONFIG.subdomain_alphabet.chars().take(CONFIG.subdomain_length).collect::<String>();
        let domain = format!("test.{}.{}.", subdomain, CONFIG.server_domain);
        
        // Set up the test record in the cache with the exact key format the handler looks for
        let _ = cache.set(&format!("dns:A:{}", domain), "1.2.3.4").await;
        
        // Create a custom server that binds to the test port directly
        let server_cache = cache.clone();
        let server_tx = tx.clone();
        let server_handle = tokio::spawn(async move {
            let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await.unwrap();
            let handler = DnsRequestHandler {
                cache: server_cache,
                tx: server_tx,
            };
            
            let mut server = ServerFuture::new(handler);
            server.register_socket(socket);
            
            let _ = server.block_until_done().await;
        });
        
        // Give the server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        let stream = UdpClientStream::<UdpSocket>::new(address);
        let connect_future = DnsExchange::connect::<_, _, TokioTime>(stream);
        let (mut client, bg) = connect_future.await.unwrap();
        tokio::spawn(bg);
        
        let name = Name::from_ascii(&domain).unwrap();
        let mut message = Message::new();
        message.set_id(rand::random::<u16>());
        message.set_message_type(MessageType::Query);
        message.add_query(Query::query(name, RecordType::A));
        
        let request = DnsRequest::new(message, DnsRequestOptions::default());
        let mut response_stream = DnsHandle::send(&mut client, request);
        
        let response = response_stream.next().await;
        assert!(response.is_some(), "No DNS response received");
        let response = response.unwrap().unwrap();
        
        let answers = response.answers();
        assert!(!answers.is_empty(), "DNS response contains no answers");
        
        if let Some(record) = answers.first() {
            if let Some(data) = record.data() {
                if let RData::A(ip) = data {
                    assert_eq!(*ip, trust_dns_proto::rr::rdata::A(Ipv4Addr::new(1, 2, 3, 4)));
                } else {
                    panic!("Expected A record");
                }
            }
        }
        
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_dns_custom_record() {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        
        // Use a different port for this test to avoid conflicts
        let port = 5354;
        
        // Set up a test record in the cache
        let domain = "test.abcdefgh.example.com.";
        let _ = cache.set(&format!("dns:A:{}", domain), "5.6.7.8").await;
        
        std::env::set_var("DOMAIN", "example.com");
        
        // Create a custom server that binds to the test port directly
        let server_cache = cache.clone();
        let server_tx = tx.clone();
        let server_handle = tokio::spawn(async move {
            let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await.unwrap();
            let handler = DnsRequestHandler {
                cache: server_cache,
                tx: server_tx,
            };
            
            let mut server = ServerFuture::new(handler);
            server.register_socket(socket);
            
            let _ = server.block_until_done().await;
        });
        
        // Give the server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        let stream = UdpClientStream::<UdpSocket>::new(address);
        let connect_future = DnsExchange::connect::<_, _, TokioTime>(stream);
        let (mut client, bg) = connect_future.await.unwrap();
        tokio::spawn(bg);
        
        let name = Name::from_ascii("test.abcdefgh.example.com.").unwrap();
        let mut message = Message::new();
        message.set_id(rand::random::<u16>());
        message.set_message_type(MessageType::Query);
        message.add_query(Query::query(name, RecordType::A));
        
        let request = DnsRequest::new(message, DnsRequestOptions::default());
        let mut response_stream = DnsHandle::send(&mut client, request);
        
        let response = response_stream.next().await;
        assert!(response.is_some(), "No DNS response received");
        let response = response.unwrap().unwrap();
        
        let answers = response.answers();
        assert!(!answers.is_empty(), "DNS response contains no answers");
        
        if let Some(record) = answers.first() {
            if let Some(data) = record.data() {
                if let RData::A(ip) = data {
                    assert_eq!(*ip, trust_dns_proto::rr::rdata::A(Ipv4Addr::new(5, 6, 7, 8)));
                } else {
                    panic!("Expected A record");
                }
            } else {
                panic!("No record data");
            }
        } else {
            panic!("No answers");
        }
        
        server_handle.abort();
    }
}
