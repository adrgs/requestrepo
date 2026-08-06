use crate::cache::Cache;
use crate::models::NotificationSettings;
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info, warn};

const NOTIFICATION_KEY_PREFIX: &str = "notifications:";

pub async fn get_settings(cache: &Arc<Cache>, subdomain: &str) -> NotificationSettings {
    let key = format!("{NOTIFICATION_KEY_PREFIX}{subdomain}");
    match cache.get(&key).await {
        Ok(Some(json_str)) => serde_json::from_str(&json_str).unwrap_or_default(),
        _ => NotificationSettings::default(),
    }
}

pub async fn save_settings(
    cache: &Arc<Cache>,
    subdomain: &str,
    settings: &NotificationSettings,
) -> Result<(), String> {
    let key = format!("{NOTIFICATION_KEY_PREFIX}{subdomain}");
    let json_str =
        serde_json::to_string(settings).map_err(|e| format!("Serialization error: {e}"))?;
    cache
        .set(&key, &json_str)
        .await
        .map_err(|e| format!("Cache error: {e}"))
}

pub async fn send_discord(message: &str, title: &str, webhook_url: &str) -> bool {
    if webhook_url.is_empty() {
        return false;
    }
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "embeds": [{
            "title": title,
            "description": message,
            "color": 0x00FF00
        }]
    });
    match client.post(webhook_url).json(&payload).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                true
            } else {
                warn!("Discord webhook returned status: {}", resp.status());
                false
            }
        }
        Err(e) => {
            error!("Failed to send Discord notification: {e}");
            false
        }
    }
}

pub async fn send_mattermost(message: &str, title: &str, webhook_url: &str) -> bool {
    if webhook_url.is_empty() {
        return false;
    }
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "text": format!("**{title}**\n{message}")
    });
    match client.post(webhook_url).json(&payload).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                true
            } else {
                warn!("Mattermost webhook returned status: {}", resp.status());
                false
            }
        }
        Err(e) => {
            error!("Failed to send Mattermost notification: {e}");
            false
        }
    }
}

pub async fn send_telegram(message: &str, title: &str, bot_token: &str, chat_id: &str) -> bool {
    if bot_token.is_empty() || chat_id.is_empty() {
        return false;
    }
    let client = reqwest::Client::new();
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": format!("<b>{title}</b>\n{message}"),
        "parse_mode": "HTML"
    });
    match client.post(&url).json(&payload).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                true
            } else {
                warn!("Telegram API returned status: {}", resp.status());
                false
            }
        }
        Err(e) => {
            error!("Failed to send Telegram notification: {e}");
            false
        }
    }
}

pub async fn send_test(cache: &Arc<Cache>, subdomain: &str, service: &str) -> Result<(), String> {
    let settings = get_settings(cache, subdomain).await;
    let msg = "This is a test notification from RequestRepo";
    let title = "RequestRepo Test Notification";
    match service.to_lowercase().as_str() {
        "discord" => {
            if settings.discord_webhook_url.is_empty() {
                return Err("Discord webhook URL not configured".to_string());
            }
            if send_discord(msg, title, &settings.discord_webhook_url).await {
                Ok(())
            } else {
                Err("Failed to send Discord notification".to_string())
            }
        }
        "mattermost" => {
            if settings.mattermost_webhook_url.is_empty() {
                return Err("Mattermost webhook URL not configured".to_string());
            }
            if send_mattermost(msg, title, &settings.mattermost_webhook_url).await {
                Ok(())
            } else {
                Err("Failed to send Mattermost notification".to_string())
            }
        }
        "telegram" => {
            if settings.telegram_bot_token.is_empty() || settings.telegram_chat_id.is_empty() {
                return Err("Telegram bot token or chat ID not configured".to_string());
            }
            if send_telegram(
                msg,
                title,
                &settings.telegram_bot_token,
                &settings.telegram_chat_id,
            )
            .await
            {
                Ok(())
            } else {
                Err("Failed to send Telegram notification".to_string())
            }
        }
        _ => Err(format!("Unknown service: {service}")),
    }
}

fn format_http_request(req: &Value) -> String {
    let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("N/A");
    let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("N/A");
    let ip = req.get("ip").and_then(|v| v.as_str()).unwrap_or("N/A");
    let port = req.get("port").and_then(|v| v.as_i64()).unwrap_or(0);
    let date = req.get("date").and_then(|v| v.as_i64()).unwrap_or(0);
    let country = req.get("country").and_then(|v| v.as_str()).unwrap_or("N/A");
    let headers_str = req
        .get("headers")
        .and_then(|v| v.as_object())
        .map(|h| {
            h.iter()
                .map(|(k, v)| format!("{k}: {}", v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    format!(
        "**HTTP Request Received**\n\
         Method: `{method}`\n\
         Path: `{path}`\n\
         IP: `{ip}`\n\
         Port: `{port}`\n\
         Time: `{date}`\n\
         Country: `{country}`\n\
         Headers:\n```\n{headers_str}\n```"
    )
}

fn format_dns_request(req: &Value) -> String {
    let query_type = req
        .get("query_type")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let domain = req.get("domain").and_then(|v| v.as_str()).unwrap_or("N/A");
    let ip = req.get("ip").and_then(|v| v.as_str()).unwrap_or("N/A");
    let port = req.get("port").and_then(|v| v.as_i64()).unwrap_or(0);
    let date = req.get("date").and_then(|v| v.as_i64()).unwrap_or(0);
    let country = req.get("country").and_then(|v| v.as_str()).unwrap_or("N/A");
    format!(
        "**DNS Request Received**\n\
         Type: `{query_type}`\n\
         Name: `{domain}`\n\
         IP: `{ip}`\n\
         Port: `{port}`\n\
         Time: `{date}`\n\
         Country: `{country}`"
    )
}

fn format_smtp_request(req: &Value) -> String {
    let from = req.get("from").and_then(|v| v.as_str()).unwrap_or("N/A");
    let to = req.get("to").and_then(|v| v.as_str()).unwrap_or("N/A");
    let subject = req.get("subject").and_then(|v| v.as_str()).unwrap_or("N/A");
    let command = req.get("command").and_then(|v| v.as_str()).unwrap_or("N/A");
    let ip = req.get("ip").and_then(|v| v.as_str()).unwrap_or("N/A");
    let port = req.get("port").and_then(|v| v.as_i64()).unwrap_or(0);
    let date = req.get("date").and_then(|v| v.as_i64()).unwrap_or(0);
    let country = req.get("country").and_then(|v| v.as_str()).unwrap_or("N/A");
    format!(
        "**SMTP Request Received**\n\
         From: `{from}`\n\
         To: `{to}`\n\
         Subject: `{subject}`\n\
         Command: `{command}`\n\
         IP: `{ip}`\n\
         Port: `{port}`\n\
         Time: `{date}`\n\
         Country: `{country}`"
    )
}

pub async fn send_request_notification(
    cache: &Arc<Cache>,
    subdomain: &str,
    service: &str,
    log: &Value,
    message: &str,
    title: &str,
) -> Result<(), String> {
    let settings = get_settings(cache, subdomain).await;
    let formatted = match log.get("type").and_then(|v| v.as_str()) {
        Some("http") => format_http_request(log),
        Some("dns") => format_dns_request(log),
        Some("smtp") => format_smtp_request(log),
        _ => message.to_string(),
    };
    match service.to_lowercase().as_str() {
        "discord" => {
            if settings.discord_webhook_url.is_empty() {
                return Err("Discord not configured".to_string());
            }
            if send_discord(&formatted, title, &settings.discord_webhook_url).await {
                Ok(())
            } else {
                Err("Failed to send Discord notification".to_string())
            }
        }
        "mattermost" => {
            if settings.mattermost_webhook_url.is_empty() {
                return Err("Mattermost not configured".to_string());
            }
            if send_mattermost(&formatted, title, &settings.mattermost_webhook_url).await {
                Ok(())
            } else {
                Err("Failed to send Mattermost notification".to_string())
            }
        }
        "telegram" => {
            if settings.telegram_bot_token.is_empty() || settings.telegram_chat_id.is_empty() {
                return Err("Telegram not configured".to_string());
            }
            if send_telegram(
                &formatted,
                title,
                &settings.telegram_bot_token,
                &settings.telegram_chat_id,
            )
            .await
            {
                Ok(())
            } else {
                Err("Failed to send Telegram notification".to_string())
            }
        }
        _ => Err(format!("Unknown service: {service}")),
    }
}

pub async fn notify_all(cache: Arc<Cache>, subdomain: String, log: Value) {
    let settings = get_settings(&cache, &subdomain).await;

    if settings.discord_webhook_url.is_empty()
        && settings.mattermost_webhook_url.is_empty()
        && (settings.telegram_bot_token.is_empty() || settings.telegram_chat_id.is_empty())
    {
        return;
    }

    let formatted = match log.get("type").and_then(|v| v.as_str()) {
        Some("http") => format_http_request(&log),
        Some("dns") => format_dns_request(&log),
        Some("smtp") => format_smtp_request(&log),
        _ => return,
    };

    info!("Sending notifications for {subdomain}");

    let title = "RequestRepo Notification";

    if !settings.discord_webhook_url.is_empty() {
        let url = settings.discord_webhook_url.clone();
        let msg = formatted.clone();
        let t = title.to_string();
        tokio::spawn(async move {
            send_discord(&msg, &t, &url).await;
        });
    }

    if !settings.mattermost_webhook_url.is_empty() {
        let url = settings.mattermost_webhook_url.clone();
        let msg = formatted.clone();
        let t = title.to_string();
        tokio::spawn(async move {
            send_mattermost(&msg, &t, &url).await;
        });
    }

    if !settings.telegram_bot_token.is_empty() && !settings.telegram_chat_id.is_empty() {
        let token = settings.telegram_bot_token.clone();
        let chat_id = settings.telegram_chat_id.clone();
        let msg = formatted;
        let t = title.to_string();
        tokio::spawn(async move {
            send_telegram(&msg, &t, &token, &chat_id).await;
        });
    }
}
