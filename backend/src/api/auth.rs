use axum::{extract::State, Json};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
    pub enable_totp: Option<bool>,
}

#[derive(Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct PasswordRequest {
    pub password: String,
}

/// 检查是否已初始化
pub async fn check_auth(State(state): State<Arc<AppState>>) -> Json<Value> {
    let initialized = crate::auth::is_initialized(&state.db).await;
    Json(json!({"ok": true, "initialized": initialized}))
}

/// 首次运行 Setup
pub async fn setup(State(state): State<Arc<AppState>>, Json(body): Json<SetupRequest>) -> (CookieJar, Json<Value>) {
    let jar = CookieJar::new();

    if crate::auth::is_initialized(&state.db).await {
        return (jar, Json(json!({"ok": false, "error": "管理员已存在，请直接登录"})));
    }

    if body.password.len() < 8 {
        return (jar, Json(json!({"ok": false, "error": "密码至少8位"})));
    }

    match crate::auth::setup_admin(&state.db, &body.username, &body.password, body.enable_totp.unwrap_or(false)).await {
        Ok((user_id, totp_secret)) => {
            // 自动登录
            match crate::auth::create_session(&state.db, user_id, true).await {
                Ok(token) => {
                    let cookie = axum_extra::extract::cookie::Cookie::parse_encoded(
                        format!("session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400", token)
                    ).unwrap();
                    let jar = jar.add(cookie);

                    let mut resp = json!({"ok": true, "message": "设置完成"});
                    if let Some(secret) = totp_secret {
                        let uri = crate::totp::get_otpauth_uri(&body.username, &secret);
                        resp["totp_secret"] = json!(secret);
                        resp["totp_uri"] = json!(uri);
                    }
                    (jar, Json(resp))
                }
                Err(e) => (jar, Json(json!({"ok": false, "error": e.to_string()}))),
            }
        }
        Err(e) => (jar, Json(json!({"ok": false, "error": e.to_string()}))),
    }
}

/// 登录
pub async fn login(State(state): State<Arc<AppState>>, Json(body): Json<LoginRequest>) -> (CookieJar, Json<Value>) {
    let jar = CookieJar::new();

    match crate::auth::login(&state.db, &body.username, &body.password).await {
        Ok((token, needs_mfa, _totp_secret)) => {
            let cookie = axum_extra::extract::cookie::Cookie::parse_encoded(
                format!("session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400", token)
            ).unwrap();
            let jar = jar.add(cookie);

            if needs_mfa {
                (jar, Json(json!({"ok": true, "needs_mfa": true, "message": "需要二次验证"})))
            } else {
                (jar, Json(json!({"ok": true, "needs_mfa": false, "message": "登录成功"})))
            }
        }
        Err(e) => (jar, Json(json!({"ok": false, "error": e.to_string()}))),
    }
}

/// TOTP 验证
pub async fn verify_totp(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(body): Json<TotpVerifyRequest>,
) -> Json<Value> {
    let token = match jar.get("session") {
        Some(c) => c.value().to_string(),
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    match crate::auth::verify_totp(&state.db, &token, &body.code).await {
        Ok(true) => Json(json!({"ok": true, "message": "验证成功"})),
        Ok(false) => Json(json!({"ok": false, "error": "验证码错误"})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 登出
pub async fn logout(jar: CookieJar, State(state): State<Arc<AppState>>) -> (CookieJar, Json<Value>) {
    if let Some(cookie) = jar.get("session") {
        let _ = sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(cookie.value())
            .execute(&state.db)
            .await;
    }
    let cookie = axum_extra::extract::cookie::Cookie::from("session");
    let jar = jar.remove(cookie);
    (jar, Json(json!({"ok": true, "message": "已退出"})))
}

/// 修改密码
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(body): Json<ChangePasswordRequest>,
) -> Json<Value> {
    let token = match jar.get("session") {
        Some(c) => c.value().to_string(),
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM sessions WHERE token = ? AND expires_at > datetime('now')"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let user_id = match user_id {
        Some(id) => id,
        None => return Json(json!({"ok": false, "error": "会话无效"})),
    };

    match crate::auth::change_password(&state.db, user_id, &body.old_password, &body.new_password).await {
        Ok(_) => Json(json!({"ok": true, "message": "密码已修改，请重新登录"})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 重置 TOTP（先验证密码，返回新密钥）
pub async fn reset_totp(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(body): Json<PasswordRequest>,
) -> Json<Value> {
    let token = match jar.get("session") {
        Some(c) => c.value().to_string(),
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM sessions WHERE token = ? AND expires_at > datetime('now')"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let user_id = match user_id {
        Some(id) => id,
        None => return Json(json!({"ok": false, "error": "会话无效"})),
    };

    // 验证密码
    let password_hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    match password_hash {
        Some(hash) => {
            if !crate::crypto::verify_password(&body.password, &hash) {
                return Json(json!({"ok": false, "error": "密码错误"}));
            }
        }
        None => return Json(json!({"ok": false, "error": "用户不存在"})),
    }

    // 生成新 TOTP 密钥
    let secret = crate::totp::generate_secret();
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_default();

    // 保存新密钥（暂不启用，等验证通过后启用）
    sqlx::query("UPDATE users SET totp_secret = ? WHERE id = ?")
        .bind(&secret)
        .bind(user_id)
        .execute(&state.db)
        .await
        .ok();

    let uri = crate::totp::get_otpauth_uri(&username, &secret);

    Json(json!({
        "ok": true,
        "totp_secret": secret,
        "totp_uri": uri,
        "message": "请扫描二维码并输入验证码完成绑定"
    }))
}

/// 禁用 TOTP
pub async fn disable_totp(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(body): Json<PasswordRequest>,
) -> Json<Value> {
    let token = match jar.get("session") {
        Some(c) => c.value().to_string(),
        None => return Json(json!({"ok": false, "error": "未登录"})),
    };

    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM sessions WHERE token = ? AND expires_at > datetime('now')"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let user_id = match user_id {
        Some(id) => id,
        None => return Json(json!({"ok": false, "error": "会话无效"})),
    };

    // 验证密码
    let password_hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    match password_hash {
        Some(hash) => {
            if !crate::crypto::verify_password(&body.password, &hash) {
                return Json(json!({"ok": false, "error": "密码错误"}));
            }
        }
        None => return Json(json!({"ok": false, "error": "用户不存在"})),
    }

    sqlx::query("UPDATE users SET totp_secret = NULL WHERE id = ?")
        .bind(user_id)
        .execute(&state.db)
        .await
        .ok();

    Json(json!({"ok": true, "message": "TOTP 已禁用"}))
}
