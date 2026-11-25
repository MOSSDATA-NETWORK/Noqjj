use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

#[derive(Deserialize)]
pub struct WhitelistReq {
    pub host_id: i64,
    pub vmid: String,
}

#[derive(Deserialize)]
pub struct SettingsReq {
    pub clean_count: i64,
    pub days: i64,
}

/// 批量磁盘扫描：所有「待检测」VM 入队（跳过白名单），后台执行
pub async fn create_batch(State(state): State<Arc<AppState>>) -> Json<Value> {
    let targets = match crate::diskscan::needs_disk_targets(&state.db).await {
        Ok(t) => t,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };
    if targets.is_empty() {
        return Json(json!({"ok": false, "error": "没有待检测的 VM"}));
    }
    match crate::diskscan::enqueue(&state, "batch", None, targets, true).await {
        Ok(o) if o.job_id.is_some() => {
            let msg = format!("已加入后台队列 {} 台（白名单跳过 {} 台，已在队列 {} 台）", o.queued, o.whitelisted, o.duplicated);
            Json(json!({"ok": true, "data": o, "message": msg}))
        }
        Ok(o) => Json(json!({"ok": false, "error": format!(
            "没有需要扫描的 VM（白名单跳过 {} 台，已在队列 {} 台）", o.whitelisted, o.duplicated
        )})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

/// 进行中的任务 + 排队/扫描中的 VM + 最近一次结束的任务 + 待检测计数（前端轮询）
pub async fn active(State(state): State<Arc<AppState>>) -> Json<Value> {
    match crate::diskscan::active_overview(&state.db).await {
        Ok(v) => Json(json!({"ok": true, "data": v})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn get(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    match crate::diskscan::get_job(&state.db, id).await {
        Ok((job, tasks)) => Json(json!({"ok": true, "data": {"job": job, "tasks": tasks}})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn stop(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    match crate::diskscan::stop_job(&state.db, id).await {
        Ok(true) => Json(json!({"ok": true, "message": "已停止：未开始的 VM 已取消，正在扫描的那台会扫完"})),
        Ok(false) => Json(json!({"ok": false, "error": "任务不存在或已结束"})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn whitelist_list(State(state): State<Arc<AppState>>) -> Json<Value> {
    match crate::diskscan::list_whitelist(&state.db).await {
        Ok(list) => Json(json!({"ok": true, "data": list})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn whitelist_add(State(state): State<Arc<AppState>>, Json(body): Json<WhitelistReq>) -> Json<Value> {
    let vmid = body.vmid.trim();
    if !crate::diskscan::is_valid_vmid(vmid) {
        return Json(json!({"ok": false, "error": "VMID 非法"}));
    }
    if let Err(e) = db::get_host(&state.db, body.host_id).await {
        return Json(json!({"ok": false, "error": e.to_string()}));
    }
    match crate::diskscan::whitelist_add(&state.db, body.host_id, vmid).await {
        Ok(_) => Json(json!({"ok": true, "message": format!("VM {} 已加入磁盘扫描白名单", vmid)})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn whitelist_remove(State(state): State<Arc<AppState>>, Path((host_id, vmid)): Path<(i64, String)>) -> Json<Value> {
    match crate::diskscan::whitelist_remove(&state.db, host_id, &vmid).await {
        Ok(_) => Json(json!({"ok": true, "message": format!("VM {} 已移出磁盘扫描白名单", vmid)})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn settings_get(State(state): State<Arc<AppState>>) -> Json<Value> {
    let (clean_count, days) = crate::diskscan::get_settings(&state.db).await;
    Json(json!({"ok": true, "data": {"clean_count": clean_count, "days": days}}))
}

pub async fn settings_update(State(state): State<Arc<AppState>>, Json(body): Json<SettingsReq>) -> Json<Value> {
    if !(0..=100).contains(&body.clean_count) {
        return Json(json!({"ok": false, "error": "连续干净次数需在 0～100 之间（0 = 不自动加入）"}));
    }
    if !(1..=3650).contains(&body.days) {
        return Json(json!({"ok": false, "error": "有效天数需在 1～3650 之间"}));
    }
    match crate::diskscan::save_settings(&state.db, body.clean_count, body.days).await {
        Ok(_) => Json(json!({"ok": true, "data": {"clean_count": body.clean_count, "days": body.days}})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}
