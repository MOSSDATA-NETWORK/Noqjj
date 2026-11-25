use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

#[derive(Deserialize)]
pub struct ScanVmRequest {
    pub vmid: String,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    match db::list_hosts(&state.db).await {
        Ok(hosts) => {
            let public: Vec<_> = hosts.iter().map(|h| h.to_public()).collect();
            Json(json!({"ok": true, "data": public}))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn get(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    match db::get_host(&state.db, id).await {
        Ok(host) => Json(json!({"ok": true, "data": host.to_public()})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn create(State(state): State<Arc<AppState>>, Json(body): Json<db::CreateHost>) -> Json<Value> {
    let host = match db::create_host(&state.db, body, &state.master_key).await {
        Ok(h) => h,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    // 自动部署检测脚本
    let auth = make_ssh_auth(&host, &state.master_key);
    match crate::deploy::deploy_script(&host.host, host.port as u16, &host.username, &auth).await {
        Ok(_) => {
            let _ = db::update_host_agent_status(&state.db, host.id, true).await;
            Json(json!({"ok": true, "data": host.to_public(), "message": "主机已添加，检测脚本已部署"}))
        }
        Err(e) => {
            Json(json!({"ok": true, "data": host.to_public(), "warning": format!("主机已添加，但脚本部署失败: {}", e)}))
        }
    }
}

pub async fn update(State(state): State<Arc<AppState>>, Path(id): Path<i64>, Json(body): Json<db::UpdateHost>) -> Json<Value> {
    match db::update_host(&state.db, id, body, &state.master_key).await {
        Ok(host) => Json(json!({"ok": true, "data": host.to_public()})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    match db::delete_host(&state.db, id).await {
        Ok(_) => Json(json!({"ok": true})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn test_connection(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    let host = match db::get_host(&state.db, id).await {
        Ok(h) => h,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    let auth = make_ssh_auth(&host, &state.master_key);
    match crate::deploy::deploy_script(&host.host, host.port as u16, &host.username, &auth).await {
        Ok(_) => {
            let _ = db::update_host_status(&state.db, id, "online").await;
            let _ = db::update_host_agent_status(&state.db, id, true).await;
            Json(json!({"ok": true, "message": "连接成功，检测脚本已部署"}))
        }
        Err(e) => {
            let _ = db::update_host_status(&state.db, id, "offline").await;
            Json(json!({"ok": false, "error": format!("连接失败: {}", e)}))
        }
    }
}

pub async fn deploy(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Json<Value> {
    let host = match db::get_host(&state.db, id).await {
        Ok(h) => h,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    let auth = make_ssh_auth(&host, &state.master_key);
    match crate::deploy::deploy_script(&host.host, host.port as u16, &host.username, &auth).await {
        Ok(_) => {
            let _ = db::update_host_agent_status(&state.db, host.id, true).await;
            Json(json!({"ok": true, "message": "检测脚本已部署"}))
        }
        Err(e) => Json(json!({"ok": false, "error": format!("部署失败: {}", e)})),
    }
}

/// 单台 VM 磁盘扫描：交给后台队列执行（网页切走不影响），立即返回任务 ID；手动单台不跳过白名单
pub async fn scan_vm(State(state): State<Arc<AppState>>, Path(id): Path<i64>, Json(body): Json<ScanVmRequest>) -> Json<Value> {
    if let Err(e) = db::get_host(&state.db, id).await {
        return Json(json!({"ok": false, "error": e.to_string()}));
    }
    let vmid = body.vmid.trim().to_string();
    if !crate::diskscan::is_valid_vmid(&vmid) {
        return Json(json!({"ok": false, "error": "VMID 非法"}));
    }
    match crate::diskscan::enqueue(&state, "single", None, vec![(id, vmid.clone())], false).await {
        Ok(o) if o.job_id.is_some() => Json(json!({"ok": true, "data": o, "message": format!("VM {} 已加入磁盘扫描队列，后台执行", vmid)})),
        Ok(_) => Json(json!({"ok": false, "error": format!("VM {} 已在磁盘扫描队列中", vmid)})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

fn make_ssh_auth(host: &db::Host, master_key: &[u8]) -> crate::deploy::SshAuth {
    crate::deploy::SshAuth::from_host(
        host.password_encrypted.as_deref(),
        host.ssh_key_encrypted.as_deref(),
        master_key,
    )
}
