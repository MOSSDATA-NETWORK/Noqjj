use axum::{extract::{Query, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

#[derive(Deserialize)]
pub struct ListParams {
    pub host_id: Option<i64>,
}

pub async fn list(State(state): State<Arc<AppState>>, Query(params): Query<ListParams>) -> Json<Value> {
    match db::list_results(&state.db, params.host_id).await {
        Ok(results) => Json(json!({"ok": true, "data": results})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    let total_hosts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hosts")
        .fetch_one(&state.db).await.unwrap_or(0);
    let online_hosts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hosts WHERE status='online'")
        .fetch_one(&state.db).await.unwrap_or(0);
    let total_scans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scans")
        .fetch_one(&state.db).await.unwrap_or(0);
    let active_threats: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE status IN ('detected','confirmed')")
        .fetch_one(&state.db).await.unwrap_or(0);
    let total_vms_scanned: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(total_vms),0) FROM scans WHERE status='completed'")
        .fetch_one(&state.db).await.unwrap_or(0);

    Json(json!({
        "ok": true,
        "data": {
            "total_hosts": total_hosts,
            "online_hosts": online_hosts,
            "total_scans": total_scans,
            "active_threats": active_threats,
            "total_vms_scanned": total_vms_scanned,
        }
    }))
}
