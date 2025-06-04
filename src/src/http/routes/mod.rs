use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use std::net::SocketAddr;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::http::AppState;
use crate::ip2country::lookup_country;
use crate::models::{CacheMessage, DnsRecords, HttpRequestLog, Response as ResponseModel};
use crate::utils::{
    generate_jwt, generate_request_id, get_current_timestamp, get_random_subdomain,
    get_subdomain_from_hostname, get_subdomain_from_path, verify_jwt, write_basic_file,
};
use crate::utils::config::CONFIG;

pub mod tcp;

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct RequestQuery {
    id: String,
    token: String,
}

pub async fn update_dns(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    Json(records): Json<DnsRecords>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let dns_record_types = ["A", "AAAA", "CNAME", "TXT"];
    
    for record in &records.records {
        let record_type = &record.r#type;
        if !dns_record_types.contains(&record_type.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": format!("Invalid record type '{}'", record_type)})),
            )
                .into_response();
        }

        let domain = record.domain.to_lowercase();
        if domain.contains(|c: char| !c.is_alphanumeric() && c != '.' && c != '-') {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": format!("Invalid characters in domain '{}'", domain)})),
            )
                .into_response();
        }
    }

    if let Ok(Some(old_records_json)) = state.cache.get(&format!("dns:{}", subdomain)).await {
        if let Ok(old_records) = serde_json::from_str::<Vec<HashMap<String, String>>>(&old_records_json) {
            for old_record in old_records {
                if let (Some(record_type), Some(domain)) = (old_record.get("type"), old_record.get("domain")) {
                    let _ = state.cache.delete(&format!("dns:{}:{}", record_type, domain)).await;
                }
            }
        }
    }

    let mut final_records = Vec::new();
    let mut values = HashMap::<String, Vec<String>>::new();

    for record in records.records {
        let new_domain = format!(
            "{}.{}.{}.",
            record.domain.to_lowercase(),
            subdomain,
            CONFIG.server_domain
        );
        
        let record_type_str = &record.r#type;
        let value = record.value.clone();
        
        let _ = state.cache.set(&format!("dns:{}:{}", record_type_str, new_domain), &value).await;
        
        values
            .entry(format!("{}:{}", record_type_str, new_domain))
            .or_default()
            .push(value.clone());
        
        final_records.push(HashMap::from([
            ("type".to_string(), record_type_str.to_string()),
            ("domain".to_string(), new_domain),
            ("value".to_string(), value),
        ]));
    }

    let _ = state.cache.set(&format!("dns:{}", subdomain), &serde_json::to_string(&final_records).unwrap()).await;

    (StatusCode::OK, Json(json!({"msg": "DNS updated"}))).into_response()
}

pub async fn get_dns(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let records = match state.cache.get(&format!("dns:{}", subdomain)).await {
        Ok(Some(records)) => records,
        _ => "[]".to_string(),
    };

    let records: Value = serde_json::from_str(&records).unwrap_or(json!([]));

    (StatusCode::OK, Json(records)).into_response()
}

pub async fn get_file(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    path: Path<String>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let file = match state.cache.get(&format!("file:{}:{}", subdomain, path.as_str())).await {
        Ok(Some(file)) => file,
        _ => "{}".to_string(),
    };

    let file: Value = serde_json::from_str(&file).unwrap_or(json!({}));

    (StatusCode::OK, Json(file)).into_response()
}

pub async fn get_request(
    State(state): State<AppState>,
    Query(params): Query<RequestQuery>,
) -> impl IntoResponse {
    let request_id = params.id;
    
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let index = match state.cache.get(&format!("request:{}:{}", subdomain, request_id)).await {
        Ok(Some(index)) => index.parse::<i64>().unwrap_or(0),
        _ => 0,
    };

    let request = match state.cache.lrange(&format!("requests:{}", subdomain), index as isize, index as isize).await {
        Ok(requests) if !requests.is_empty() => requests[0].clone(),
        _ => "{}".to_string(),
    };

    let request: Value = serde_json::from_str(&request).unwrap_or(json!({}));

    (StatusCode::OK, Json(request)).into_response()
}

pub async fn delete_request(
    State(state): State<AppState>,
    Query(params): Query<RequestQuery>,
) -> impl IntoResponse {
    let request_id = params.id;
    
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let index = match state.cache.get(&format!("request:{}:{}", subdomain, request_id)).await {
        Ok(Some(index)) => index.parse::<i64>().unwrap_or(0),
        _ => 0,
    };

    let _ = state.cache.lset(&format!("requests:{}", subdomain), index as isize, "{}").await;
    let _ = state.cache.delete(&format!("request:{}:{}", subdomain, request_id)).await;

    let message = CacheMessage {
        cmd: "delete_request".to_string(),
        subdomain: subdomain.clone(),
        data: request_id,
    };

    let _ = state.tx.send(message);

    (StatusCode::OK, Json(json!({"msg": "Request deleted"}))).into_response()
}

pub async fn delete_all(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let keys = match state.cache.keys(&format!("request:{}:*", subdomain)).await {
        Ok(keys) => keys,
        Err(_) => Vec::new(),
    };

    for key in &keys {
        let _ = state.cache.delete(key).await;
    }

    let _ = state.cache.delete(&format!("requests:{}", subdomain)).await;

    let message = CacheMessage {
        cmd: "delete_all".to_string(),
        subdomain: subdomain.clone(),
        data: "".to_string(),
    };

    let _ = state.tx.send(message);

    (StatusCode::OK, Json(json!({"msg": "All requests deleted"}))).into_response()
}

pub async fn update_file(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    path: Path<String>,
    Json(file): Json<ResponseModel>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let _ = state.cache.set(&format!("file:{}:{}", subdomain, path.as_str()), &serde_json::to_string(&file).unwrap()).await;

    (StatusCode::OK, Json(json!({"msg": "File updated"}))).into_response()
}

pub async fn get_token(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Option<Json<Value>>,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();
    let rate_limit_key = format!("rate_limit:token:{}", client_ip);
    
    if let Ok(Some(count)) = state.cache.get(&rate_limit_key).await {
        if let Ok(count) = count.parse::<u32>() {
            if count >= 10 {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"detail": "Rate limit exceeded. Try again later."})),
                ).into_response();
            }
        }
    }
    
    let current_count = match state.cache.get(&rate_limit_key).await {
        Ok(Some(count)) => count.parse::<u32>().unwrap_or(0) + 1,
        _ => 1,
    };
    
    let _ = state.cache.set(&rate_limit_key, &current_count.to_string()).await;
    let _ = state.cache.set(&format!("{}:ttl", rate_limit_key), &(get_current_timestamp() + 60).to_string()).await;
    let subdomain = match request {
        Some(Json(req)) => match req.get("subdomain") {
            Some(subdomain) => match subdomain.as_str() {
                Some(subdomain) => subdomain.to_string(),
                None => get_random_subdomain(),
            },
            None => get_random_subdomain(),
        },
        None => get_random_subdomain(),
    };

    let token = match generate_jwt(&subdomain) {
        Ok(token) => token,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": "Failed to generate token"})),
            )
                .into_response();
        }
    };

    if let Err(e) = write_basic_file(&subdomain, &state.cache).await {
        error!("Failed to write basic file: {}", e);
    }

    (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "subdomain": subdomain
        })),
    )
        .into_response()
}

pub async fn get_requests(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let requests = match state.cache.lrange(&format!("requests:{}", subdomain), 0, -1).await {
        Ok(requests) => {
            let requests: Vec<Value> = requests
                .into_iter()
                .filter(|r| r != "{}")
                .filter_map(|r| serde_json::from_str::<Value>(&r).ok())
                .collect();
            requests
        },
        _ => Vec::new(),
    };

    (StatusCode::OK, Json(requests)).into_response()
}

pub async fn get_files(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let files = match state.cache.get(&format!("files:{}", subdomain)).await {
        Ok(Some(files)) => files,
        _ => {
            let _ = crate::utils::write_basic_file(&subdomain, &state.cache).await;
            match state.cache.get(&format!("files:{}", subdomain)).await {
                Ok(Some(files)) => files,
                _ => "{}".to_string(),
            }
        }
    };

    let files: Value = serde_json::from_str(&files).unwrap_or(json!({}));

    (StatusCode::OK, Json(files)).into_response()
}

pub async fn update_files(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    Json(tree): Json<HashMap<String, Value>>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };
    
    fn validate_tree(tree: &HashMap<String, Value>) -> Result<(), String> {
        fn validate_tree_recursive(tree_dict: &HashMap<String, Value>, path: &str) -> Result<(), String> {
            for (key, value) in tree_dict {
                let current_path = format!("{}{}", path, key);
                
                if value.is_string() {
                    continue;
                }
                
                if let Some(obj) = value.as_object() {
                    if key.ends_with("/") {
                        let dir_map = obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<HashMap<String, Value>>();
                        validate_tree_recursive(&dir_map, &current_path)?;
                    } else {
                        if !obj.contains_key("raw") || !obj.contains_key("headers") || !obj.contains_key("status_code") {
                            return Err(format!("Invalid file structure for {}", current_path));
                        }
                        
                        if !obj["raw"].is_string() {
                            return Err(format!("Invalid raw file structure for {}", current_path));
                        }
                        
                        if !obj["headers"].is_array() {
                            return Err(format!("Invalid headers file structure for {}", current_path));
                        }
                        
                        if !obj["status_code"].is_number() {
                            return Err(format!("Invalid status_code file structure for {}", current_path));
                        }
                    }
                } else {
                    return Err(format!("Invalid structure for {}", current_path));
                }
            }
            Ok(())
        }
        
        validate_tree_recursive(tree, "")
    }
    
    if let Err(err) = validate_tree(&tree) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response();
    }
    
    let existing_files = match state.cache.get(&format!("files:{}", subdomain)).await {
        Ok(Some(files)) => {
            match serde_json::from_str::<HashMap<String, Value>>(&files) {
                Ok(map) => map,
                Err(_) => HashMap::new(),
            }
        },
        _ => {
            let _ = crate::utils::write_basic_file(&subdomain, &state.cache).await;
            match state.cache.get(&format!("files:{}", subdomain)).await {
                Ok(Some(files)) => {
                    match serde_json::from_str::<HashMap<String, Value>>(&files) {
                        Ok(map) => map,
                        Err(_) => HashMap::new(),
                    }
                },
                _ => HashMap::new(),
            }
        }
    };
    
    let mut merged_files = existing_files.clone();
    for (key, value) in tree.iter() {
        if key == "index.html" && value.is_null() {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "index.html cannot be deleted"}))).into_response();
        }
        
        if let Some(content) = value.as_str() {
            let response = json!({
                "raw": BASE64.encode(content.as_bytes()),
                "headers": [
                    {
                        "header": "Content-Type",
                        "value": "text/plain"
                    }
                ],
                "status_code": 200
            });
            merged_files.insert(key.clone(), response);
        } else {
            merged_files.insert(key.clone(), value.clone());
        }
    }
    
    if !merged_files.contains_key("index.html") {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "index.html cannot be deleted"}))).into_response();
    }
    
    let _ = state.cache.set(&format!("files:{}", subdomain), &serde_json::to_string(&merged_files).unwrap()).await;

    (StatusCode::OK, Json(json!({"msg": "Updated files"}))).into_response()
}

pub async fn catch_all(
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Body,
) -> impl IntoResponse {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    
    let path = uri.path();
    
    let subdomain = get_subdomain_from_hostname(host)
        .or_else(|| get_subdomain_from_path(path));
    
    if let Some(subdomain) = subdomain.clone() {
        use futures_util::StreamExt;
        let max_request_size = 1024 * 1024 * 10; // 10MB limit
        let mut body_bytes = Vec::new();
        let mut body_stream = body.into_data_stream();
        
        while let Some(chunk) = body_stream.next().await {
            match chunk {
                Ok(data) => {
                    body_bytes.extend_from_slice(&data);
                    if body_bytes.len() > max_request_size {
                        return Response::builder()
                            .status(StatusCode::PAYLOAD_TOO_LARGE)
                            .body(Body::from("Request body too large"))
                            .unwrap()
                            .into_response();
                    }
                }
                Err(_) => break,
            }
        }
        
        let request_id = generate_request_id();
        
        let (client_ip, client_port) = if let Some(forwarded) = headers.get("x-forwarded-for") {
            let forwarded_str = forwarded.to_str().unwrap_or("127.0.0.1");
            let ip = forwarded_str.split(',').next().unwrap_or("127.0.0.1").trim();
            (ip.to_string(), addr.port() as i32)
        } else {
            (addr.ip().to_string(), addr.port() as i32)
        };
        
        let http_version = "1.1"; // Default since Axum doesn't expose this easily
        let scheme = if uri.scheme_str() == Some("https") || headers.get("x-forwarded-proto").map(|h| h.to_str().unwrap_or("")) == Some("https") {
            "HTTPS"
        } else {
            "HTTP"
        };
        let protocol = format!("{}/{}", scheme, http_version);
        
        let fragment = String::new();
        
        let query = if let Some(q) = uri.query() {
            q.to_string()
        } else {
            String::new()
        };
        let url_query = if !query.is_empty() {
            format!("?{}", query)
        } else {
            String::new()
        };
        
        let url = format!("{}://{}{}{}{}", 
            scheme.to_lowercase(), 
            host, 
            path,
            url_query,
            fragment
        );
        
        let country = lookup_country(&client_ip);
        
        let request_log = HttpRequestLog {
            _id: request_id.clone(),
            r#type: "http".to_string(),
            raw: BASE64.encode(&body_bytes),
            uid: subdomain.clone(),
            method: method.to_string(),
            path: path.to_string(),
            headers: {
                let mut header_map = HashMap::new();
                
                for (name, value) in headers.iter() {
                    let header_name = name.to_string();
                    let header_value = value.to_str().unwrap_or("").to_string();
                    
                    header_map.insert(header_name, header_value);
                }
                
                header_map
            },
            date: get_current_timestamp(),
            ip: Some(client_ip),
            country,
            port: client_port,
            protocol,
            fragment,
            query,
            url,
        };
        
        let request_json = serde_json::to_string(&request_log).unwrap_or_default();
        
        let _ = state.cache.rpush(&format!("requests:{}", subdomain), &request_json).await;
        let _ = state.cache.set(&format!("request:{}:{}", subdomain, request_id), "0").await;
        
        let message = crate::models::CacheMessage {
            cmd: "new_request".to_string(),
            subdomain: subdomain.clone(),
            data: request_json,
        };
        
        let _ = state.tx.send(message);
        
        return serve_file(state, subdomain, path).await;
    }
    
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
        .into_response()
}

async fn serve_file(state: AppState, subdomain: String, path: &str) -> Response {
    let files = match state.cache.get(&format!("files:{}", subdomain)).await {
        Ok(Some(files)) => files,
        _ => {
            let _ = crate::utils::write_basic_file(&subdomain, &state.cache).await;
            match state.cache.get(&format!("files:{}", subdomain)).await {
                Ok(Some(files)) => files,
                _ => "{}".to_string(),
            }
        }
    };
    
    let files: HashMap<String, ResponseModel> = serde_json::from_str(&files).unwrap_or_default();
    
    let file_path = path.trim_start_matches('/');
    let file_path = if file_path.is_empty() { "index.html" } else { file_path };
    
    if let Some(file) = files.get(file_path) {
        let content = match BASE64.decode(&file.raw) {
            Ok(content) => content,
            Err(_) => Vec::new(),
        };
        
        let mut response = Response::builder()
            .status(StatusCode::from_u16(file.status_code).unwrap_or(StatusCode::OK));
        
        for header in &file.headers {
            if let Ok(name) = HeaderName::from_str(&header.header) {
                if let Ok(value) = HeaderValue::from_str(&header.value) {
                    response = response.header(name, value);
                }
            }
        }
        
        return response
            .body(Body::from(content))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal server error"))
                    .unwrap()
            })
            .into_response();
    }
    
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
        .into_response()
}
