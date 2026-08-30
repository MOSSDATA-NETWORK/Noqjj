mod api;
mod auth;
mod crypto;
mod db;
mod deploy;
mod detect;
mod notify;
mod passkey;
mod ratelimit;
mod scheduler;
mod totp;
mod version;

use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, AllowOrigin, AllowMethods, AllowHeaders};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub master_key: Vec<u8>,
    pub tls_enabled: bool,
    pub login_limiter: ratelimit::RateLimiter,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:chicken-detect.db?mode=rwc".to_string());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // 运行数据库迁移
    sqlx::query(include_str!("../migrations/001_init.sql"))
        .execute(&pool)
        .await?;

    // 加密密钥：优先从环境变量读取，否则自动生成并打印
    let master_key = load_or_generate_master_key();

    // 检查 TLS 配置
    let tls_cert = std::env::var("TLS_CERT").ok();
    let tls_key = std::env::var("TLS_KEY").ok();
    let tls_enabled = tls_cert.is_some() && tls_key.is_some();

    let state = Arc::new(AppState {
        db: pool.clone(),
        master_key,
        tls_enabled,
        login_limiter: ratelimit::RateLimiter::new(5, 300), // 5次/5分钟
    });

    // 启动定时任务调度器
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = scheduler::start(scheduler_state).await {
            tracing::error!("Scheduler error: {}", e);
        }
    });

    // 定期清理过期 session
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            auth::cleanup_sessions(&cleanup_pool).await;
        }
    });

    let static_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| "static".to_string());

    let port = std::env::var("PORT").unwrap_or_else(|_| "3210".to_string());
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    // CORS：只允许同源
    let origin_str = if tls_enabled { "https" } else { "http" };
    let default_origin = format!("{}://{}:{}", origin_str, host, port);
    let cors_origin = std::env::var("CORS_ORIGIN").unwrap_or(default_origin);
    let origin: axum::http::HeaderValue = cors_origin.parse()
        .map_err(|e| anyhow::anyhow!("CORS_ORIGIN 格式错误: {}", e))?;

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::exact(origin))
        .allow_methods(AllowMethods::list([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
        ]))
        .allow_headers(AllowHeaders::list([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::COOKIE,
        ]))
        .allow_credentials(true);

    let app = Router::new()
        .nest("/api", api::routes(state.clone()))
        .fallback_service(ServeDir::new(&static_dir))
        .layer(cors);

    let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse()?;

    if let (Some(cert_path), Some(key_path)) = (&tls_cert, &tls_key) {
        tracing::info!("Noqjj v{} running on https://{}:{} (TLS enabled)", version::VERSION, host, port);
        let tls_config = load_tls_config(cert_path, key_path).await?;
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        tracing::warn!("Noqjj v{} running on http://{}:{} (无 TLS，建议配置 HTTPS)", version::VERSION, host, port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}

/// 加载或生成加密密钥
fn load_or_generate_master_key() -> Vec<u8> {
    if let Ok(key_hex) = std::env::var("MASTER_KEY") {
        // 从环境变量加载
        match hex_to_bytes(&key_hex) {
            Ok(key) if key.len() == 32 => {
                tracing::info!("已从环境变量加载加密密钥");
                return key;
            }
            _ => {
                tracing::error!("MASTER_KEY 环境变量格式错误，需要64位hex字符（32字节）");
                std::process::exit(1);
            }
        }
    }

    // 自动生成并打印
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    let hex = bytes_to_hex(&key);

    eprintln!("");
    eprintln!("╔══════════════════════════════════════════════════╗");
    eprintln!("║           加密密钥已自动生成                      ║");
    eprintln!("╠══════════════════════════════════════════════════╣");
    eprintln!("║ MASTER_KEY={}", hex);
    eprintln!("║");
    eprintln!("║ 请保存此密钥！后续启动必须设置此环境变量。");
    eprintln!("║ 丢失密钥将无法解密已存储的凭据。");
    eprintln!("╚══════════════════════════════════════════════════╝");
    eprintln!("");

    key.to_vec()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_to_bytes(hex: &str) -> anyhow::Result<Vec<u8>> {
    if hex.len() % 2 != 0 || hex.len() != 64 {
        return Err(anyhow::anyhow!("长度错误"));
    }
    Ok((0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()?)
}

async fn load_tls_config(cert_path: &str, key_path: &str) -> anyhow::Result<axum_server::tls_rustls::RustlsConfig> {
    let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| anyhow::anyhow!("TLS 配置失败: {}", e))?;
    Ok(config)
}
