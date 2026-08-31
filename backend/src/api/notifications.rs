use axum::{extract::{Path, State}, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

/// 脱敏配置：隐藏敏感字段
fn mask_config(raw: &str, notify_type: &str) -> String {
    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(raw) {
        match notify_type {
            "telegram" => {
                if let Some(token) = val.get_mut("bot_token") {
                    if let Some(s) = token.as_str() {
                        if s.len() > 8 {
                            *token = serde_json::Value::String(format!("{}...{}", &s[..4], &s[s.len()-4..]));
                        }
                    }
                }
            }
            "wecom" => {
                if let Some(wh) = val.get_mut("webhook") {
                    if let Some(s) = wh.as_str() {
                        if s.len() > 20 {
                            *wh = serde_json::Value::String(format!("{}...{}", &s[..16], &s[s.len()-4..]));
                        }
                    }
                }
            }
            _ => {}
        }
        serde_json::to_string(&val).unwrap_or_else(|_| "***".to_string())
    } else {
        "***".to_string()
    }
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    match db::list_notifications(&state.db).await {
        Ok(items) => {
            let public: Vec<db::NotificationPublic> = items.iter().map(|n| {
                let config = crate::crypto::decrypt(&n.config_encrypted, &state.master_key)
                    .map(|c| mask_config(&c, &n.r#type))
                    .unwrap_or_else(|_| "***".to_string());
                db::NotificationPublic {
                    id: n.id,
                    r#type: n.r#type.clone(),
                    enabled: n.enabled,
                    config,
                    notify_level: n.notify_level.clone(),
                    created_at: n.created_at,
                    updated_at: n.updated_at,
                }
            }).collect();
            Json(json!({"ok": true, "data": public}))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn create(State(state): State<Arc<AppState>>, Json(body): Json<db::CreateNotification>) -> Json<Value> {
    match db::upsert_notification(&state.db, None, body, &state.master_key).await {
        Ok(n) => Json(json!({"ok": true, "data": {"id": n.id, "type": n.r#type}})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn update(State(state): State<Arc<AppState>>, Path(id): Path<i64>, Json(body): Json<db::CreateNotification>) -> Json<Value> {
    match db::upsert_notification(&state.db, Some(id), body, &state.master_key).await {
        Ok(n) => Json(json!({"ok": true, "data": {"id": n.id, "type": n.r#type}})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn test(State(state): State<Arc<AppState>>) -> Json<Value> {
    let notifications = match db::list_notifications(&state.db).await {
        Ok(n) => n,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    let mut results = Vec::new();
    for n in &notifications {
        if !n.enabled { continue; }
        let config = match crate::crypto::decrypt(&n.config_encrypted, &state.master_key) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let r = match n.r#type.as_str() {
            "telegram" => crate::notify::test_telegram(&config).await,
            "wecom" => crate::notify::test_wecom(&config).await,
            _ => Ok("未知类型".to_string()),
        };
        results.push(json!({
            "type": n.r#type,
            "success": r.is_ok(),
            "message": r.unwrap_or_else(|e| e.to_string()),
        }));
    }

    Json(json!({"ok": true, "data": results}))
}
