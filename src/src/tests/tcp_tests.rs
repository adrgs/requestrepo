use crate::cache::Cache;
use crate::models::{CacheMessage, TcpRequestLog};
use crate::tcp::Server as TcpServer;
use std::sync::Arc;
use tokio::sync::broadcast;
use base64::Engine;

#[tokio::test]
async fn test_tcp_port_allocation() {
    let cache = Arc::new(Cache::new());
    let (tx, _) = broadcast::channel::<CacheMessage>(100);
    let tx = Arc::new(tx);

    let server = TcpServer::new(cache.clone(), tx.clone());

    let subdomain = "test_subdomain";
    let port = server.allocate_port(subdomain).await.unwrap();

    assert!(port >= 10000 && port <= 11000);

    let allocated_subdomain = server.get_subdomain_for_port(port).await;
    assert_eq!(allocated_subdomain, Some(subdomain.to_string()));

    server.release_port(subdomain).await.unwrap();

    let allocated_subdomain = server.get_subdomain_for_port(port).await;
    assert_eq!(allocated_subdomain, None);
}

#[tokio::test]
async fn test_tcp_response_handling() {
    let cache = Arc::new(Cache::new());
    let (tx, _) = broadcast::channel::<CacheMessage>(100);
    let tx = Arc::new(tx);

    let server = TcpServer::new(cache.clone(), tx.clone());

    let subdomain = "test_subdomain";
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nHello, World!";
    
    cache.set(&format!("tcp_response:{}", subdomain), response).await.unwrap();

    let stored_response = cache.get(&format!("tcp_response:{}", subdomain)).await.unwrap().unwrap();
    assert_eq!(stored_response, response);
}

#[tokio::test]
async fn test_tcp_request_logging() {
    let cache = Arc::new(Cache::new());
    let (tx, _) = broadcast::channel::<CacheMessage>(100);
    let tx = Arc::new(tx);

    let server = TcpServer::new(cache.clone(), tx.clone());

    let subdomain = "test_subdomain";
    let port = 10001;
    let request_data = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    
    let request_log = TcpRequestLog {
        _id: "test_id".to_string(),
        r#type: "tcp".to_string(),
        raw: base64::engine::general_purpose::STANDARD.encode(request_data),
        uid: subdomain.to_string(),
        port,
        date: chrono::Utc::now().timestamp(),
        ip: Some("127.0.0.1".to_string()),
        country: Some("Unknown".to_string()),
    };
    
    let request_json = serde_json::to_string(&request_log).unwrap();
    
    cache.rpush(&format!("tcp_requests:{}", subdomain), &request_json).await.unwrap();
    
    let stored_requests = cache.lrange(&format!("tcp_requests:{}", subdomain), 0, -1).await.unwrap();
    assert_eq!(stored_requests.len(), 1);
    
    let stored_request: TcpRequestLog = serde_json::from_str(&stored_requests[0]).unwrap();
    assert_eq!(stored_request.port, port);
    assert_eq!(stored_request.uid, subdomain);
}
