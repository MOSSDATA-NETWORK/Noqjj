mod api;
mod auth;
mod crypto;
mod db;
mod deploy;
mod detect;
mod notify;
mod passkey;
mod scheduler;
mod totp;

use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub master_key: Vec<u8>,
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

    // 获取或生成加密密钥（自动管理，不需要环境变量）
    let master_key = crypto::get_or_create_master_key(&pool).await?;

    let state = Arc::new(AppState {
        db: pool.clone(),
        master_key,
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

    let app = Router::new()
        .nest("/api", api::routes(state.clone()))
        .fallback_service(ServeDir::new(&static_dir))
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Chicken Detect running on http://0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
