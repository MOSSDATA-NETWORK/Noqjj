use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

#[derive(Deserialize)]
pub struct CreateReq {
    pub host_id: Option<i64>,
    pub cron_expr: String,
    pub enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateReq {
    pub cron_expr: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    match db::list_schedules(&state.db).await {
        Ok(s) => Json(json!({"ok": true, "data": s})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn create(State(state): State<Arc<AppState>>, Json(body): Json<CreateReq>) -> Json<Value> {
    // 校验 cron 表达式并预计算下次执行时间
    let next = match crate::scheduler::compute_next_run(&body.cron_expr) {
        Some(n) => n,
        None => return Json(json!({"ok": false, "error": "cron 表达式无效，支持 5 段（分 时 日 月 周）或 6 段（含秒）"})),
    };
    let s = match db::create_schedule(&state.db, db::CreateSchedule {
        host_id: body.host_id,
        cron_expr: body.cron_expr.trim().to_string(),
        enabled: body.enabled,
    }).await {
        Ok(s) => s,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };
    let _ = db::set_schedule_next_run(&state.db, s.id, Some(next)).await;
    Json(json!({"ok": true, "data": s}))
}

pub async fn update(State(state): State<Arc<AppState>>, Path(id): Path<i64>, Json(body): Json<UpdateReq>) -> Json<Value> {
    if let Some(expr) = &body.cron_expr {
        if crate::scheduler::compute_next_run(expr).is_none() {
            return Json(json!({"ok": false, "error": "cron 表达式无效"}));
        }
    }
    let s = match db::update_schedule(&state.db, id, body.cron_expr.as_deref(), body.enabled).await {
        Ok(s) => s,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };
    // cron 或启用状态变化后重新计算下次执行时间
    let next = if s.enabled { crate::scheduler::compute_next_run(&s.cron_expr) } else { None };
    let _ = db::set_schedule_next_run(&state.db, id, next).await;
    Json(json!({"ok": true, "data": s}))
}

pub async fn remove(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    match db::delete_schedule(&state.db, id).await {
        Ok(_) => Json(json!({"ok": true, "message": "已删除"})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 立即执行一次该定时任务对应的扫描
pub async fn run_now(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    let schedules = match db::list_schedules(&state.db).await {
        Ok(s) => s,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };
    let schedule = match schedules.into_iter().find(|s| s.id == id) {
        Some(s) => s,
        None => return Json(json!({"ok": false, "error": "定时任务不存在"})),
    };

    let scan = match db::create_scan(&state.db, schedule.host_id).await {
        Ok(s) => s,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };
    let state_clone = state.clone();
    let scan_id = scan.id;
    let host_id = schedule.host_id;
    tokio::spawn(async move {
        if let Err(e) = crate::detect::run_scan(state_clone, scan_id, host_id).await {
            tracing::error!("Manual scheduled scan {} failed: {}", scan_id, e);
        }
    });
    let next = crate::scheduler::compute_next_run(&schedule.cron_expr);
    let _ = db::update_schedule_run(&state.db, id, next).await;
    Json(json!({"ok": true, "data": scan, "message": "扫描已启动"}))
}
