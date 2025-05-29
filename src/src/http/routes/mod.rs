use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
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
use crate::models::{CacheMessage, DnsRecords, FileTree, HttpRequestLog, Response as ResponseModel};
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
    subdomain: String,
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
        if !dns_record_types.contains(&record.r#type.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": format!("Invalid record type '{}'", record.r#type)})),
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
        
        let record_type = record.r#type.clone();
        let value = record.value.clone();
        
        let _ = state.cache.set(&format!("dns:{}:{}", record_type, new_domain), &value).await;
        
        values
            .entry(format!("{}:{}", record_type, new_domain))
            .or_default()
            .push(value.clone());
        
        final_records.push(HashMap::from([
            ("type".to_string(), record_type),
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
    let subdomain = params.subdomain;

    if !verify_jwt(&subdomain).is_some() {
        return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
    }

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
    let subdomain = params.subdomain;

    if !verify_jwt(&subdomain).is_some() {
        return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
    }

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
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let subdomain = match request.get("subdomain") {
        Some(subdomain) => match subdomain.as_str() {
            Some(subdomain) => subdomain.to_string(),
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
        _ => "{}".to_string(),
    };

    let files: Value = serde_json::from_str(&files).unwrap_or(json!({}));

    (StatusCode::OK, Json(files)).into_response()
}

pub async fn update_files(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    Json(files): Json<FileTree>,
) -> impl IntoResponse {
    let subdomain = match verify_jwt(&params.token) {
        Some(subdomain) => subdomain,
        None => {
            return (StatusCode::FORBIDDEN, Json(json!({"detail": "Invalid token"}))).into_response();
        }
    };

    let _ = state.cache.set(&format!("files:{}", subdomain), &serde_json::to_string(&files).unwrap()).await;

    (StatusCode::OK, Json(json!({"msg": "Updated files"}))).into_response()
}

pub async fn catch_all(
    State(state): State<AppState>,
    uri: Uri,
    method: axum::http::Method,
    headers: HeaderMap,
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
        let body_bytes = match axum::body::to_bytes(body, 1024 * 1024 * 10).await {
            Ok(bytes) => bytes.to_vec(),
            Err(_) => Vec::new(),
        };
        
        let request_id = generate_request_id();
        
        let client_ip = headers
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        
        let country = lookup_country(&client_ip);
        
        let request_log = HttpRequestLog {
            _id: request_id.clone(),
            r#type: "http".to_string(),
            raw: BASE64.encode(&body_bytes),
            uid: subdomain.clone(),
            method: method.to_string(),
            path: path.to_string(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
            date: get_current_timestamp(),
            ip: Some(client_ip),
            country,
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
        _ => "{}".to_string(),
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
