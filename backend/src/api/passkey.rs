/// Passkey API 端点
use axum::{extract::State, Json};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct PasskeyLoginStartRequest {
    pub username: String,
}

#[derive(Deserialize)]
pub struct PasskeyLoginFinishRequest {
    pub username: String,
    pub credential: serde_json::Value,
}

/// Passkey 注册：生成挑战
pub async fn register_start(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Json<Value> {
    let user_id = match get_user_id(&state, &jar).await {
        Some(id) => id,
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_default();

    let challenge = crate::passkey::generate_challenge();

    // 存储 challenge 到 session（简化：存到 settings 表）
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('passkey_reg_challenge', ?)")
        .bind(&challenge)
        .execute(&state.db)
        .await
        .ok();

    // 获取已有的凭据 ID（避免重复注册）
    let existing_creds = crate::passkey::get_user_credential_ids(&state.db, &username).await;

    Json(json!({
        "ok": true,
        "challenge": challenge,
        "rp": {
            "id": crate::passkey::RP_ID,
            "name": crate::passkey::RP_NAME,
        },
        "user": {
            "id": user_id.to_string(),
            "name": username,
            "displayName": username,
        },
        "excludeCredentials": existing_creds,
    }))
}

/// Passkey 注册：验证并存储
pub async fn register_finish(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(resp): Json<serde_json::Value>,
) -> Json<Value> {
    let user_id = match get_user_id(&state, &jar).await {
        Some(id) => id,
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    // 获取存储的 challenge
    let challenge: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'passkey_reg_challenge'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or_default();

    // 解析响应
    let reg_resp: crate::passkey::RegistrationResponse = match serde_json::from_value(resp) {
        Ok(r) => r,
        Err(e) => return Json(json!({"ok": false, "error": format!("响应格式错误: {}", e)})),
    };

    match crate::passkey::verify_and_store_credential(&state.db, user_id, &reg_resp, &challenge).await {
        Ok(_) => {
            // 清除 challenge
            sqlx::query("DELETE FROM settings WHERE key = 'passkey_reg_challenge'")
                .execute(&state.db)
                .await
                .ok();
            Json(json!({"ok": true, "message": "Passkey 注册成功"}))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// Passkey 登录：生成挑战
pub async fn login_start(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PasskeyLoginStartRequest>,
) -> Json<Value> {
    let challenge = crate::passkey::generate_challenge();

    // 存储 challenge
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('passkey_auth_challenge', ?)")
        .bind(&challenge)
        .execute(&state.db)
        .await
        .ok();

    // 获取用户的凭据 ID
    let allow_creds = crate::passkey::get_user_credential_ids(&state.db, &body.username).await;

    if allow_creds.is_empty() {
        return Json(json!({"ok": false, "error": "该用户未注册 Passkey"}));
    }

    Json(json!({
        "ok": true,
        "challenge": challenge,
        "rpId": crate::passkey::RP_ID,
        "allowCredentials": allow_creds,
    }))
}

/// Passkey 登录：验证签名
pub async fn login_finish(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PasskeyLoginFinishRequest>,
) -> (CookieJar, Json<Value>) {
    let jar = CookieJar::new();

    // 获取存储的 challenge
    let challenge: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'passkey_auth_challenge'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or_default();

    // 解析响应
    let auth_resp: crate::passkey::AuthenticationResponse = match serde_json::from_value(body.credential) {
        Ok(r) => r,
        Err(e) => return (jar, Json(json!({"ok": false, "error": format!("响应格式错误: {}", e)}))),
    };

    match crate::passkey::verify_authentication(&state.db, &auth_resp, &challenge).await {
        Ok(Some(user_id)) => {
            // 清除 challenge
            sqlx::query("DELETE FROM settings WHERE key = 'passkey_auth_challenge'")
                .execute(&state.db)
                .await
                .ok();

            // 创建 session
            match crate::auth::create_session(&state.db, user_id, true).await {
                Ok(token) => {
                    let cookie = axum_extra::extract::cookie::Cookie::parse_encoded(
                        format!("session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400", token)
                    ).unwrap();
                    let jar = jar.add(cookie);
                    (jar, Json(json!({"ok": true, "message": "Passkey 登录成功"})))
                }
                Err(e) => (jar, Json(json!({"ok": false, "error": e.to_string()}))),
            }
        }
        Ok(None) => (jar, Json(json!({"ok": false, "error": "未找到匹配的凭据"}))),
        Err(e) => (jar, Json(json!({"ok": false, "error": e.to_string()}))),
    }
}

/// 删除 Passkey 凭据
pub async fn delete(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Json<Value> {
    let user_id = match get_user_id(&state, &jar).await {
        Some(id) => id,
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    match crate::passkey::delete_credentials(&state.db, user_id).await {
        Ok(_) => Json(json!({"ok": true, "message": "Passkey 已删除"})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 检查用户是否有 Passkey
pub async fn has_passkey(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PasskeyLoginStartRequest>,
) -> Json<Value> {
    let cred_ids = crate::passkey::get_user_credential_ids(&state.db, &body.username).await;
    Json(json!({
        "ok": true,
        "has_passkey": !cred_ids.is_empty(),
        "credential_count": cred_ids.len(),
    }))
}

async fn get_user_id(state: &AppState, jar: &CookieJar) -> Option<i64> {
    let token = jar.get("session")?.value();
    sqlx::query_scalar("SELECT user_id FROM sessions WHERE token = ? AND expires_at > datetime('now')")
        .bind(token)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
}
