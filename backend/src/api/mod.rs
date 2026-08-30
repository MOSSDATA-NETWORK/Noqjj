pub mod auth;
pub mod hosts;
pub mod notifications;
pub mod results;
pub mod scans;

use axum::{Router, middleware, routing::{get, post, put}};
use std::sync::Arc;
use crate::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    // 公开路由
    let public = Router::new()
        .route("/auth/check", get(auth::check_auth))
        .route("/auth/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify-totp", post(auth::verify_totp))
        .route("/auth/logout", post(auth::logout))
        .with_state(state.clone());

    // 需要认证的路由
    let protected = Router::new()
        .route("/hosts", get(hosts::list).post(hosts::create))
        .route("/hosts/{id}", get(hosts::get).put(hosts::update).delete(hosts::delete))
        .route("/hosts/{id}/test", post(hosts::test_connection))
        .route("/hosts/{id}/deploy", post(hosts::deploy))
        .route("/scans", get(scans::list).post(scans::create))
        .route("/scans/{id}", get(scans::get))
        .route("/results", get(results::list))
        .route("/results/stats", get(results::stats))
        .route("/notifications", get(notifications::list).post(notifications::create))
        .route("/notifications/{id}", put(notifications::update))
        .route("/notifications/test", post(notifications::test))
        .route("/auth/password", post(auth::change_password))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), crate::auth::auth_middleware));

    public.merge(protected)
}
