use axum::{extract::{Path, State}, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, db};

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
    // 创建主机记录
    let host = match db::create_host(&state.db, body, &state.master_key).await {
        Ok(h) => h,
        Err(e) => return Json(json!({"ok": false, "error": e.to_string()})),
    };

    // 自动部署检测脚本
    let password = decrypt_host_password(&host, &state.master_key);
    match crate::deploy::deploy_script(&host.host, host.port as u16, &host.username, password.as_deref()).await {
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

    let password = decrypt_host_password(&host, &state.master_key);
    match crate::deploy::deploy_script(&host.host, host.port as u16, &host.username, password.as_deref()).await {
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

    let password = decrypt_host_password(&host, &state.master_key);
    match crate::deploy::deploy_script(&host.host, host.port as u16, &host.username, password.as_deref()).await {
        Ok(_) => {
            let _ = db::update_host_agent_status(&state.db, id, true).await;
            Json(json!({"ok": true, "message": "检测脚本已部署"}))
        }
        Err(e) => Json(json!({"ok": false, "error": format!("部署失败: {}", e)})),
    }
}

fn decrypt_host_password(host: &db::Host, master_key: &[u8]) -> Option<String> {
    host.password_encrypted.as_ref().and_then(|enc| {
        crate::crypto::decrypt(enc, master_key).ok()
    })
}
