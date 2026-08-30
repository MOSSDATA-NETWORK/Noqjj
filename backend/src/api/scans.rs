use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

#[derive(Deserialize)]
pub struct CreateScanRequest {
    pub host_id: Option<i64>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    match db::list_scans(&state.db).await {
        Ok(scans) => Json(json!({"ok": true, "data": scans})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn get(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    match sqlx::query_as::<_, db::Scan>("SELECT * FROM scans WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(scan)) => Json(json!({"ok": true, "data": scan})),
        Ok(None) => Json(json!({"ok": false, "error": "扫描不存在"})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn create(State(state): State<Arc<AppState>>, Json(body): Json<CreateScanRequest>) -> Json<Value> {
    // Create scan record
    let scan = match db::create_scan(&state.db, body.host_id).await {
        Ok(s) => s,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    // Run scan in background
    let state_clone = state.clone();
    let scan_id = scan.id;
    let host_id = body.host_id;
    tokio::spawn(async move {
        if let Err(e) = crate::detect::run_scan(state_clone, scan_id, host_id).await {
            tracing::error!("Scan {} failed: {}", scan_id, e);
            let _ = db::fail_scan(&state.db, scan_id, &e.to_string()).await;
        }
    });

    Json(json!({"ok": true, "data": scan, "message": "扫描已启动"}))
}
