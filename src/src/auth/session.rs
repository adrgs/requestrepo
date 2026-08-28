use crate::cache::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub username: String,
    pub avatar_url: String,
    pub name: String,
    pub created_at: String,
    pub last_accessed: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub avatar_url: String,
    pub name: String,
    pub created_at: String,
    pub is_admin: bool,
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn generate_session_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &bytes)
}

pub async fn create_session(
    cache: &Arc<Cache>,
    username: &str,
    avatar_url: &str,
    name: &str,
    is_admin: bool,
) -> Result<String, String> {
    let token = generate_session_token();
    let token_hash = hash_token(&token);

    let session = UserSession {
        username: username.to_string(),
        avatar_url: avatar_url.to_string(),
        name: name.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_accessed: chrono::Utc::now().to_rfc3339(),
        is_admin,
    };

    let json = serde_json::to_string(&session)
        .map_err(|e| format!("Serialization error: {e}"))?;

    cache
        .set(&format!("session:{token_hash}"), &json)
        .await
        .map_err(|e| format!("Cache error: {e}"))?;

    // Track user sessions
    let sessions_key = format!("user_sessions:{username}");
    let existing = cache.get(&sessions_key).await.unwrap_or(None);
    let mut session_list: Vec<String> = existing
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    session_list.push(token_hash);
    let list_json = serde_json::to_string(&session_list).unwrap_or_default();
    let _ = cache.set(&sessions_key, &list_json).await;

    Ok(token)
}

pub async fn get_session(cache: &Arc<Cache>, token: &str) -> Option<UserSession> {
    let token_hash = hash_token(token);
    let key = format!("session:{token_hash}");
    let json = cache.get(&key).await.ok().flatten()?;
    let mut session: UserSession = serde_json::from_str(&json).ok()?;

    // Update last accessed
    session.last_accessed = chrono::Utc::now().to_rfc3339();
    let updated_json = serde_json::to_string(&session).ok()?;
    let _ = cache.set(&key, &updated_json).await;

    Some(session)
}

pub async fn delete_session(cache: &Arc<Cache>, token: &str) -> Result<(), String> {
    let token_hash = hash_token(token);
    let key = format!("session:{token_hash}");

    // Get session to find username
    if let Some(json) = cache.get(&key).await.map_err(|e| e.to_string())? {
        if let Ok(session) = serde_json::from_str::<UserSession>(&json) {
            // Remove from user_sessions list
            let sessions_key = format!("user_sessions:{}", session.username);
            if let Ok(Some(list_json)) = cache.get(&sessions_key).await {
                if let Ok(mut list) = serde_json::from_str::<Vec<String>>(&list_json) {
                    list.retain(|h| h != &token_hash);
                    let _ = cache.set(&sessions_key, &serde_json::to_string(&list).unwrap_or_default()).await;
                }
            }
        }
    }

    cache.delete(&key).await.map(|_| ()).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn delete_all_user_sessions(cache: &Arc<Cache>, username: &str) -> Result<(), String> {
    let sessions_key = format!("user_sessions:{username}");
    if let Ok(Some(list_json)) = cache.get(&sessions_key).await {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&list_json) {
            for token_hash in list {
                let _ = cache.delete(&format!("session:{token_hash}")).await;
            }
        }
    }
    let _ = cache.delete(&sessions_key).await;
    Ok(())
}

pub async fn register_user(
    cache: &Arc<Cache>,
    profile: &UserProfile,
) -> Result<(), String> {
    let json = serde_json::to_string(profile).map_err(|e| e.to_string())?;
    cache
        .set(&format!("registered_user:{}", profile.username), &json)
        .await
        .map_err(|e| e.to_string())?;

    // Add to registered_users set
    let users_key = "registered_users";
    let existing = cache.get(users_key).await.unwrap_or(None);
    let mut users: Vec<String> = existing
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !users.contains(&profile.username) {
        users.push(profile.username.clone());
        let list_json = serde_json::to_string(&users).unwrap_or_default();
        let _ = cache.set(users_key, &list_json).await;
    }

    Ok(())
}

pub async fn get_all_users(cache: &Arc<Cache>) -> Vec<UserProfile> {
    let users_key = "registered_users";
    let users_json = match cache.get(users_key).await {
        Ok(Some(json)) => json,
        _ => return Vec::new(),
    };

    let usernames: Vec<String> = match serde_json::from_str(&users_json) {
        Ok(u) => u,
        _ => return Vec::new(),
    };

    let mut profiles = Vec::new();
    for username in usernames {
        let key = format!("registered_user:{username}");
        if let Ok(Some(json)) = cache.get(&key).await {
            if let Ok(profile) = serde_json::from_str::<UserProfile>(&json) {
                profiles.push(profile);
            }
        }
    }
    profiles
}

#[allow(dead_code)]
pub async fn get_user_profile(cache: &Arc<Cache>, username: &str) -> Option<UserProfile> {
    let key = format!("registered_user:{username}");
    let json = cache.get(&key).await.ok().flatten()?;
    serde_json::from_str(&json).ok()
}

pub fn is_user_allowed(username: &str, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true; // No allowlist = allow all
    }
    allowed.iter().any(|u| u == username)
}

pub fn is_user_admin(username: &str, admins: &[String]) -> bool {
    admins.iter().any(|a| a == username)
}
