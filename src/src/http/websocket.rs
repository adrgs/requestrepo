//! WebSocket handler for real-time request streaming
//!
//! V2 Protocol:
//! Client → Server:
//!   {"cmd": "connect", "token": "jwt..."}
//!   {"cmd": "ping"}
//!   {"cmd": "disconnect"}
//!
//! Server → Client:
//!   {"cmd": "connected", "subdomain": "abc123"}
//!   {"cmd": "pong"}
//!   {"cmd": "request", "subdomain": "abc123", "data": {...}}
//!   {"cmd": "requests", "subdomain": "abc123", "data": [...]}
//!   {"cmd": "error", "code": "invalid_token", "message": "..."}

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::http::AppState;
use crate::utils::verify_jwt;

/// V2 WebSocket endpoint handler
pub async fn websocket_handler_v2(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket_v2(socket, state))
}

/// V2 WebSocket handler with proper JSON protocol
async fn handle_socket_v2(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    let sessions: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let sessions_for_broadcast = sessions.clone();
    let sessions_for_receive = sessions.clone();

    // Channel for sending messages to the WebSocket
    let (tx, mut rx) = mpsc::channel::<String>(100);
    let tx_for_broadcast = tx.clone();

    // Subscribe to broadcast messages
    let mut broadcast_rx = state.tx.subscribe();

    // Task to handle broadcast messages
    let broadcast_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            let subdomain = msg.subdomain.clone();
            let is_subscribed = {
                let sessions = sessions_for_broadcast.lock().await;
                sessions.contains(&subdomain)
            };

            if is_subscribed {
                // Parse data from string to JSON object (fix double-encoding)
                let data = serde_json::from_str::<Value>(&msg.data).unwrap_or(Value::Null);

                // Map cmd names: new_request -> request, delete_request -> deleted
                let cmd = match msg.cmd.as_str() {
                    "new_request" => "request",
                    "delete_request" => "deleted",
                    "delete_all" => "cleared",
                    other => other,
                };

                let ws_msg = json!({
                    "cmd": cmd,
                    "subdomain": subdomain,
                    "data": data
                });

                if tx_for_broadcast.send(ws_msg.to_string()).await.is_err() {
                    break;
                }
            }
        }
    });

    // Task to send messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(json) = serde_json::from_str::<Value>(&text) {
                    if let Some(cmd) = json.get("cmd").and_then(|c| c.as_str()) {
                        match cmd {
                            "connect" => {
                                let token = match json.get("token").and_then(|t| t.as_str()) {
                                    Some(t) => t,
                                    None => {
                                        let response = json!({
                                            "cmd": "error",
                                            "code": "missing_token",
                                            "message": "Token is required"
                                        });
                                        let _ = tx.send(response.to_string()).await;
                                        continue;
                                    }
                                };

                                match verify_jwt(token) {
                                    Some(subdomain) => {
                                        {
                                            let mut sessions = sessions_for_receive.lock().await;
                                            sessions.insert(subdomain.clone());
                                        }

                                        // Send connected confirmation
                                        let response = json!({
                                            "cmd": "connected",
                                            "subdomain": subdomain
                                        });
                                        if tx.send(response.to_string()).await.is_err() {
                                            break;
                                        }

                                        // Send historical requests
                                        if let Ok(requests) = state
                                            .cache
                                            .lrange(&format!("requests:{}", subdomain), 0, -1)
                                            .await
                                        {
                                            let requests: Vec<Value> = requests
                                                .into_iter()
                                                .filter(|r| r != "{}")
                                                .filter_map(|r| serde_json::from_str::<Value>(&r).ok())
                                                .collect();

                                            if !requests.is_empty() {
                                                let response = json!({
                                                    "cmd": "requests",
                                                    "subdomain": subdomain,
                                                    "data": requests
                                                });

                                                if tx.send(response.to_string()).await.is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        let response = json!({
                                            "cmd": "error",
                                            "code": "invalid_token",
                                            "message": "Invalid or expired token"
                                        });
                                        let _ = tx.send(response.to_string()).await;
                                    }
                                }
                            }
                            "ping" => {
                                let response = json!({"cmd": "pong"});
                                if tx.send(response.to_string()).await.is_err() {
                                    break;
                                }
                            }
                            "disconnect" => {
                                if let Some(subdomain) =
                                    json.get("subdomain").and_then(|s| s.as_str())
                                {
                                    let mut sessions = sessions_for_receive.lock().await;
                                    sessions.remove(subdomain);
                                }
                            }
                            _ => {
                                let response = json!({
                                    "cmd": "error",
                                    "code": "unknown_command",
                                    "message": format!("Unknown command: {}", cmd)
                                });
                                let _ = tx.send(response.to_string()).await;
                            }
                        }
                    }
                } else {
                    let response = json!({
                        "cmd": "error",
                        "code": "invalid_json",
                        "message": "Invalid JSON format"
                    });
                    let _ = tx.send(response.to_string()).await;
                }
            }
            Message::Close(_) => {
                break;
            }
            _ => {}
        }
    }

    broadcast_task.abort();
    send_task.abort();
}
