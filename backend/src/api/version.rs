use axum::Json;
use serde_json::{json, Value};

/// 获取当前版本
pub async fn current() -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": crate::version::VERSION
    }))
}

/// 检查更新
pub async fn check() -> Json<Value> {
    match crate::version::check_update().await {
        Ok(info) => Json(json!({"ok": true, "data": info})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 获取更新日志
pub async fn changelog() -> Json<Value> {
    match crate::version::get_changelog().await {
        Ok(entries) => Json(json!({"ok": true, "data": entries})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 执行更新
pub async fn update() -> Json<Value> {
    match crate::version::perform_update().await {
        Ok(msg) => Json(json!({"ok": true, "message": msg})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}
