/// 认证模块：Setup 流程 + 登录 + TOTP/Passkey 2FA
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use sqlx::SqlitePool;
use std::sync::Arc;
use crate::AppState;

/// 检查是否已初始化（是否有管理员账户）
pub async fn is_initialized(pool: &SqlitePool) -> bool {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    count > 0
}

/// Setup：创建管理员（首次运行）
pub async fn setup_admin(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    totp_enabled: bool,
) -> anyhow::Result<(i64, Option<String>)> {
    if is_initialized(pool).await {
        return Err(anyhow::anyhow!("管理员已存在"));
    }

    let hash = crate::crypto::hash_password(password);
    let totp_secret = if totp_enabled {
        Some(crate::totp::generate_secret())
    } else {
        None
    };

    let id = sqlx::query(
        "INSERT INTO users (username, password_hash, totp_secret, role) VALUES (?, ?, ?, 'admin')"
    )
    .bind(username)
    .bind(&hash)
    .bind(&totp_secret)
    .execute(pool)
    .await?
    .last_insert_rowid();

    Ok((id, totp_secret))
}

/// 保存 Passkey 凭据
pub async fn save_passkey(pool: &SqlitePool, user_id: i64, credential_json: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET passkey_credential = ? WHERE id = ?")
        .bind(credential_json)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 认证中间件
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // 公开路由
    if path.starts_with("/api/auth/login")
        || path.starts_with("/api/auth/check")
        || path.starts_with("/api/auth/setup")
        || path.starts_with("/api/auth/passkey/")
    {
        return next.run(request).await;
    }

    // 静态文件
    if !path.starts_with("/api/") {
        return next.run(request).await;
    }

    // 检查 session
    if let Some(cookie) = jar.get("session") {
        let token = cookie.value();
        let valid: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE token = ? AND expires_at > datetime('now')"
        )
        .bind(token)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        if valid > 0 {
            return next.run(request).await;
        }
    }

    let body = serde_json::json!({"ok": false, "error": "未登录", "code": "UNAUTHORIZED"});
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

/// 创建 session
pub async fn create_session(pool: &SqlitePool, user_id: i64, mfa_verified: bool) -> anyhow::Result<String> {
    let token = crate::crypto::generate_token();
    let expires = chrono::Utc::now().naive_utc() + chrono::Duration::hours(24);

    sqlx::query("INSERT INTO sessions (user_id, token, expires_at, mfa_verified) VALUES (?, ?, ?, ?)")
        .bind(user_id)
        .bind(&token)
        .bind(expires)
        .bind(mfa_verified)
        .execute(pool)
        .await?;

    Ok(token)
}

/// 清理过期 session
pub async fn cleanup_sessions(pool: &SqlitePool) {
    let _ = sqlx::query("DELETE FROM sessions WHERE expires_at < datetime('now')")
        .execute(pool)
        .await;
}

/// 登录（第一步：密码验证）
pub async fn login(pool: &SqlitePool, username: &str, password: &str) -> anyhow::Result<(String, bool, Option<String>)> {
    let user = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>)>(
        "SELECT id, username, password_hash, totp_secret, passkey_credential FROM users WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    let (user_id, _, hash, totp_secret, passkey_cred) = user.ok_or_else(|| anyhow::anyhow!("用户不存在"))?;

    if !crate::crypto::verify_password(password, &hash) {
        return Err(anyhow::anyhow!("密码错误"));
    }

    let has_totp = totp_secret.is_some();
    let has_passkey = passkey_cred.is_some();
    let needs_mfa = has_totp || has_passkey;

    // 如果没有2FA，直接创建已验证 session
    let mfa_verified = !needs_mfa;
    let token = create_session(pool, user_id, mfa_verified).await?;

    Ok((token, needs_mfa, totp_secret))
}

/// 验证 TOTP
pub async fn verify_totp(pool: &SqlitePool, session_token: &str, code: &str) -> anyhow::Result<bool> {
    let (user_id,): (i64,) = sqlx::query_as(
        "SELECT user_id FROM sessions WHERE token = ? AND expires_at > datetime('now')"
    )
    .bind(session_token)
    .fetch_one(pool)
    .await?;

    let totp_secret: Option<String> = sqlx::query_scalar(
        "SELECT totp_secret FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    match totp_secret {
        Some(secret) => {
            let valid = crate::totp::verify_code(&secret, code);
            if valid {
                sqlx::query("UPDATE sessions SET mfa_verified = 1 WHERE token = ?")
                    .bind(session_token)
                    .execute(pool)
                    .await?;
            }
            Ok(valid)
        }
        None => Err(anyhow::anyhow!("未启用 TOTP")),
    }
}

/// 修改密码
pub async fn change_password(pool: &SqlitePool, user_id: i64, old_password: &str, new_password: &str) -> anyhow::Result<()> {
    let (_, _, hash): (i64, String, String) = sqlx::query_as(
        "SELECT id, username, password_hash FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if !crate::crypto::verify_password(old_password, &hash) {
        return Err(anyhow::anyhow!("原密码错误"));
    }

    let new_hash = crate::crypto::hash_password(new_password);
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(user_id)
        .execute(pool)
        .await?;

    // 清除该用户所有 session
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}
