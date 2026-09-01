use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{AppState, db};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VmDetectionResult {
    pub vmid: String,
    pub method: String,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ScriptOutput {
    results: Vec<ScriptResult>,
}

#[derive(Debug, Deserialize)]
struct ScriptResult {
    vmid: String,
    method: String,
    status: String,
    evidence: Option<String>,
}

/// 运行扫描（通过远程脚本，带超时和进度更新）
pub async fn run_scan(state: Arc<AppState>, scan_id: i64, host_id: Option<i64>) -> anyhow::Result<()> {
    let hosts = match host_id {
        Some(hid) => vec![db::get_host(&state.db, hid).await?],
        None => db::list_hosts(&state.db).await?,
    };

    for host in &hosts {
        tracing::info!("Scanning host {} ({})", host.name, host.host);

        // 检查 agent 是否已部署
        if !host.agent_deployed {
            tracing::warn!("Host {} agent not deployed, skipping", host.name);
            continue;
        }

        // 构建认证方式
        let auth = crate::deploy::SshAuth::from_host(
            host.password_encrypted.as_deref(),
            host.ssh_key_encrypted.as_deref(),
            &state.master_key,
        );

        // 更新扫描状态为 running
        let _ = db::update_scan_status(&state.db, scan_id, "running").await;

        // 远程执行检测脚本（带5分钟超时）
        let scan_future = crate::deploy::run_remote_scan(
            &host.host, host.port as u16, &host.username, &auth, None
        );

        let output = match tokio::time::timeout(
            tokio::time::Duration::from_secs(300),
            scan_future,
        ).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                tracing::error!("Host {} scan failed: {}", host.name, e);
                let _ = db::update_host_status(&state.db, host.id, "error").await;
                let _ = db::fail_scan(&state.db, scan_id, &format!("扫描失败: {}", e)).await;
                continue;
            }
            Err(_) => {
                tracing::error!("Host {} scan timed out after 5 minutes", host.name);
                let _ = db::update_host_status(&state.db, host.id, "error").await;
                let _ = db::fail_scan(&state.db, scan_id, "扫描超时（5分钟）").await;
                continue;
            }
        };

        // 解析 JSON 输出
        let script_output: ScriptOutput = match serde_json::from_str(&output) {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Host {} output parse failed: {}, raw: {}", host.name, e, output);
                let _ = db::fail_scan(&state.db, scan_id, &format!("脚本输出解析失败: {}", e)).await;
                continue;
            }
        };

        let total = script_output.results.len() as i64;
        let mut ga_count: i64 = 0;
        let mut disk_count: i64 = 0;
        let mut found_count: i64 = 0;

        // 获取上次结果用于比对
        let prev_results = db::get_previous_results(&state.db, host.id).await?;
        let prev_detected: std::collections::HashSet<String> = prev_results
            .iter()
            .filter(|r| r.status == "detected" || r.status == "confirmed")
            .map(|r| r.vmid.clone())
            .collect();

        let mut current_detected = std::collections::HashSet::new();

        // 更新 VM 总数（进度反馈）
        let _ = db::update_scan_progress(&state.db, scan_id, total, 0, 0, 0).await;

        for (i, r) in script_output.results.iter().enumerate() {
            match r.method.as_str() {
                "ga" => ga_count += 1,
                "disk" => disk_count += 1,
                _ => {}
            }

            let status = if r.status == "detected" {
                found_count += 1;
                current_detected.insert(r.vmid.clone());
                if prev_detected.contains(&r.vmid) { "confirmed" } else { "detected" }
            } else if r.status == "clean" {
                if prev_detected.contains(&r.vmid) { "cleaned" } else { "clean" }
            } else {
                "unknown"
            };

            if status != "clean" {
                let evidence = r.evidence.as_deref().unwrap_or("");
                db::upsert_result(&state.db, scan_id, host.id, &r.vmid, status, &r.method, evidence).await?;

                if status == "detected" || status == "cleaned" {
                    let ev: Vec<String> = evidence.split_whitespace().map(|s| s.to_string()).collect();
                    crate::notify::send_all(&state.db, &state.master_key, status, &host.name, &r.vmid, &ev.join(" ")).await;
                }
            }

            // 每处理5个VM更新一次进度
            if (i + 1) % 5 == 0 || i + 1 == total as usize {
                let _ = db::update_scan_progress(&state.db, scan_id, total, ga_count, disk_count, found_count).await;
            }
        }

        // 标记已清除的
        for prev_vmid in &prev_detected {
            if !current_detected.contains(prev_vmid) {
                db::upsert_result(&state.db, scan_id, host.id, prev_vmid, "cleaned", "", "").await?;
                crate::notify::send_all(&state.db, &state.master_key, "cleaned", &host.name, prev_vmid, "切鸡软件已清除").await;
            }
        }

        db::complete_scan(&state.db, scan_id, total, ga_count, disk_count, found_count).await?;
        let _ = db::update_host_status(&state.db, host.id, "online").await;

        tracing::info!("Host {} scan complete: {} VMs, {} found", host.name, total, found_count);
    }

    Ok(())
}
