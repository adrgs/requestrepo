
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, error, info};

use crate::http::AppState;
use crate::models::CacheMessage;
use crate::utils::verify_jwt;

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

pub async fn websocket_handler_v2(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket_v2(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    
    let sessions = Arc::new(Mutex::new(HashSet::new()));
    let sessions_clone = sessions.clone();

    let mut rx = state.tx.subscribe();
    let sender_clone = sender.clone();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let subdomain = msg.subdomain.clone();
            let is_subscribed = {
                let sessions = sessions.lock().unwrap();
                sessions.contains(&subdomain)
            };

            if is_subscribed {
                let ws_msg = json!({
                    "cmd": msg.cmd,
                    "data": msg.data,
                    "subdomain": subdomain
                });

                let mut sender = sender_clone.lock().await;
                if let Err(e) = sender.send(Message::Text(ws_msg.to_string())).await {
                    error!("Error sending WebSocket message: {}", e);
                    break;
                }
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Some(subdomain) = verify_jwt(&text) {
                    {
                        let mut sessions = sessions_clone.lock().unwrap();
                        sessions.insert(subdomain.clone());
                    }

                    let response = json!({
                        "cmd": "connected",
                        "subdomain": subdomain
                    });

                    {
                        let mut sender_lock = sender.lock().await;
                        if let Err(e) = sender_lock.send(Message::Text(response.to_string())).await {
                            error!("Error sending WebSocket message: {}", e);
                            break;
                        }
                    }

                    if let Ok(requests) = state.cache.lrange(&format!("requests:{}", subdomain), 0, -1).await {
                        let requests: Vec<String> = requests.into_iter().filter(|r| r != "{}").collect();
                        
                        if !requests.is_empty() {
                            let response = json!({
                                "cmd": "requests",
                                "data": requests,
                                "subdomain": subdomain
                            });

                            {
                                let mut sender_lock = sender.lock().await;
                                if let Err(e) = sender_lock.send(Message::Text(response.to_string())).await {
                                    error!("Error sending WebSocket message: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    let response = json!({
                        "cmd": "invalid_token",
                        "token": text
                    });

                    {
                        let mut sender_lock = sender.lock().await;
                        if let Err(e) = sender_lock.send(Message::Text(response.to_string())).await {
                            error!("Error sending WebSocket message: {}", e);
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => {
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
}

async fn handle_socket_v2(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(tokio::sync::Mutex::new(sender));

    let sessions = Arc::new(Mutex::new(HashSet::new()));
    let sessions_clone = sessions.clone();

    let mut rx = state.tx.subscribe();
    let sender_clone = sender.clone();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let subdomain = msg.subdomain.clone();
            let is_subscribed = {
                let sessions = sessions.lock().unwrap();
                sessions.contains(&subdomain)
            };

            if is_subscribed {
                let ws_msg = json!({
                    "cmd": msg.cmd,
                    "data": serde_json::from_str::<Value>(&msg.data).unwrap_or(Value::Null),
                    "subdomain": subdomain
                });

                let mut sender_lock = sender_clone.lock().await;
                if let Err(e) = sender_lock.send(Message::Text(ws_msg.to_string())).await {
                    error!("Error sending WebSocket message: {}", e);
                    break;
                }
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(json) = serde_json::from_str::<Value>(&text) {
                    if let Some(cmd) = json.get("cmd").and_then(|c| c.as_str()) {
                        match cmd {
                            "connect" => {
                                if let Some(token) = json.get("token").and_then(|t| t.as_str()) {
                                    if let Some(subdomain) = verify_jwt(token) {
                                        {
                                            let mut sessions = sessions_clone.lock().unwrap();
                                            sessions.insert(subdomain.clone());
                                        }

                                        let response = json!({
                                            "cmd": "connected",
                                            "subdomain": subdomain
                                        });

                                        {
                                            let mut sender_lock = sender.lock().await;
                                            if let Err(e) = sender_lock.send(Message::Text(response.to_string())).await {
                                                error!("Error sending WebSocket message: {}", e);
                                                break;
                                            }
                                        }

                                        if let Ok(requests) = state.cache.lrange(&format!("requests:{}", subdomain), 0, -1).await {
                                            let requests: Vec<Value> = requests
                                                .into_iter()
                                                .filter(|r| r != "{}")
                                                .filter_map(|r| serde_json::from_str::<Value>(&r).ok())
                                                .collect();
                                            
                                            if !requests.is_empty() {
                                                let response = json!({
                                                    "cmd": "requests",
                                                    "data": requests,
                                                    "subdomain": subdomain
                                                });

                                                {
                                                    let mut sender_lock = sender.lock().await;
                                                    if let Err(e) = sender_lock.send(Message::Text(response.to_string())).await {
                                                        error!("Error sending WebSocket message: {}", e);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        let response = json!({
                                            "cmd": "invalid_token",
                                            "token": token
                                        });

                                        {
                                            let mut sender_lock = sender.lock().await;
                                            if let Err(e) = sender_lock.send(Message::Text(response.to_string())).await {
                                                error!("Error sending WebSocket message: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            "disconnect" => {
                                if let Some(subdomain) = json.get("subdomain").and_then(|s| s.as_str()) {
                                    {
                                        let mut sessions = sessions_clone.lock().unwrap();
                                        sessions.remove(subdomain);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Message::Close(_) => {
                break;
            }
            _ => {}
        }
    }

    send_task.abort();
}
