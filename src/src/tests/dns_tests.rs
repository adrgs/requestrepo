
#[cfg(test)]
mod tests {
    use crate::cache::Cache;
    use crate::dns::Server;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use tokio::net::UdpSocket;
    use tokio::sync::broadcast;
    use trust_dns_client::client::{Client, SyncClient};
    use trust_dns_client::op::DnsResponse;
    use trust_dns_client::rr::{DNSClass, Name, RData, Record, RecordType};
    use trust_dns_client::udp::UdpClientConnection;


    #[tokio::test]
    #[ignore]
    async fn test_dns_a_record() {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        
        let server = Server::new(cache.clone(), tx);
        let server_handle = tokio::spawn(async move {
            let _ = server.run().await;
        });
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 53);
        let conn = UdpClientConnection::new(address).unwrap();
        let client = SyncClient::new(conn);
        
        let name = Name::from_ascii("test.example.com.").unwrap();
        let response = client.query(&name, DNSClass::IN, RecordType::A).unwrap();
        
        let answers = response.answers();
        assert!(!answers.is_empty());
        
        server_handle.abort();
    }

    #[tokio::test]
    #[ignore]
    async fn test_dns_custom_record() {
        let cache = Arc::new(Cache::new());
        let (tx, _) = broadcast::channel(1024);
        let tx = Arc::new(tx);
        
        let _ = cache.set("dns:A:test.abcdefgh.example.com.", "5.6.7.8").await;
        
        let server = Server::new(cache.clone(), tx);
        let server_handle = tokio::spawn(async move {
            let _ = server.run().await;
        });
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 53);
        let conn = UdpClientConnection::new(address).unwrap();
        let client = SyncClient::new(conn);
        
        let name = Name::from_ascii("test.abcdefgh.example.com.").unwrap();
        let response = client.query(&name, DNSClass::IN, RecordType::A).unwrap();
        
        let answers = response.answers();
        assert!(!answers.is_empty());
        
        if let Some(record) = answers.first() {
            if let RData::A(ip) = record.rdata() {
                assert_eq!(ip, &Ipv4Addr::new(5, 6, 7, 8));
            } else {
                panic!("Expected A record");
            }
        } else {
            panic!("No answers");
        }
        
        server_handle.abort();
    }
}
