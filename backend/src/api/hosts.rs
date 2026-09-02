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

pub async fn scan_vm(State(state): State<Arc<AppState>>, Path(id): Path<i64>, Json(body): Json<ScanVmRequest>) -> Json<Value> {
    let host = match db::get_host(&state.db, id).await {
        Ok(h) => h,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    let auth = make_ssh_auth(&host, &state.master_key);

    // 执行单 VM 磁盘扫描
    match crate::deploy::run_remote_scan(&host.host, host.port as u16, &host.username, &auth, Some(&body.vmid)).await {
        Ok(output) => {
            // 解析结果并更新数据库
            tracing::info!("scan_vm {} raw output ({} bytes): {}", body.vmid, output.len(), output.chars().take(500).collect::<String>());
            match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(result) => {
                    let empty = vec![];
                    let results = result.get("results").and_then(|r| r.as_array()).unwrap_or(&empty);
                    tracing::info!("scan_vm {} parsed {} results", body.vmid, results.len());
                    for r in results {
                        let vmid = r.get("vmid").and_then(|v| v.as_str()).unwrap_or("");
                        let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let method = r.get("method").and_then(|v| v.as_str()).unwrap_or("");
                        let evidence = r.get("evidence").and_then(|v| v.as_str()).unwrap_or("");

                        let db_status = if status == "detected" { "detected" }
                            else if status == "clean" { "clean" }
                            else if status == "needs_disk_scan" { "needs_disk_scan" }
                            else { "unknown" };

                        if let Err(e) = db::upsert_result(&state.db, 0, id, vmid, db_status, method, evidence).await {
                            tracing::error!("scan_vm upsert failed: {}", e);
                        }
                    }
                }
                Err(e) => tracing::error!("scan_vm {} parse failed: {}, raw: {}", body.vmid, e, output),
            }
            Json(json!({"ok": true, "message": format!("VM {} 磁盘扫描完成", body.vmid)}))
        }
        Err(e) => Json(json!({"ok": false, "error": format!("扫描失败: {}", e)})),
    }
}

fn make_ssh_auth(host: &db::Host, master_key: &[u8]) -> crate::deploy::SshAuth {
    crate::deploy::SshAuth::from_host(
        host.password_encrypted.as_deref(),
        host.ssh_key_encrypted.as_deref(),
        master_key,
    )
}
