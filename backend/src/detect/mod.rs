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
        tracing::info!("Scan {} started for host {} ({}:{})", scan_id, host.name, host.host, host.port);

        // 先获取 VM 总数（快速，用于前端进度显示）
        let vm_count_output = crate::deploy::run_remote_cmd(
            &host.host, host.port as u16, &host.username, &auth,
            "qm list | awk 'NR>1' | wc -l"
        ).await;
        let vm_total: i64 = vm_count_output
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        if vm_total > 0 {
            let _ = db::update_scan_progress(&state.db, scan_id, vm_total, 0, 0, 0).await;
            tracing::info!("Scan {} host {} has {} VMs", scan_id, host.name, vm_total);
        }

        // 远程执行检测脚本（带超时）+ 实时进度轮询
        // 脚本每完成一个 VM 会把 "已处理数 已发现数" 写入进度文件，这里每3秒读取一次
        tracing::info!("Scan {} executing remote script on {}...", scan_id, host.host);
        let prog_file = format!("/tmp/chicken-progress-{}", scan_id);
        let scan_cmd = format!("CHICKEN_PROGRESS={} chicken-check --all", prog_file);

        // 扫描任务（独立 clone，避免借用冲突）；完成时置位 done 标志
        let auth_scan = crate::deploy::SshAuth::from_host(
            host.password_encrypted.as_deref(),
            host.ssh_key_encrypted.as_deref(),
            &state.master_key,
        );
        let (h1, p1, u1, cmd1) = (host.host.clone(), host.port as u16, host.username.clone(), scan_cmd.clone());
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_scan = done.clone();
        let scan_task = tokio::spawn(async move {
            let r = crate::deploy::ssh_exec(&h1, p1, &u1, &auth_scan, &cmd1).await;
            done_scan.store(true, std::sync::atomic::Ordering::Relaxed);
            r
        });

        // 进度轮询任务（vm_total>0 时才启用）：每3秒读一次进度文件
        let auth_poll = crate::deploy::SshAuth::from_host(
            host.password_encrypted.as_deref(),
            host.ssh_key_encrypted.as_deref(),
            &state.master_key,
        );
        let (h2, p2, u2) = (host.host.clone(), host.port as u16, host.username.clone());
        let db2 = state.db.clone();
        let total_now = vm_total;
        let cat_cmd = format!("cat {}", prog_file);
        let done_poll = done.clone();
        tokio::spawn(async move {
            if total_now <= 0 { return; }
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                if done_poll.load(std::sync::atomic::Ordering::Relaxed) { break; }
                if let Ok(prog) = crate::deploy::ssh_exec(&h2, p2, &u2, &auth_poll, &cat_cmd).await {
                    let mut it = prog.split_whitespace();
                    if let (Some(n), Some(f)) = (it.next(), it.next()) {
                        if let (Ok(n), Ok(f)) = (n.parse::<i64>(), f.parse::<i64>()) {
                            let _ = db::update_scan_progress(&db2, scan_id, total_now, n, 0, f).await;
                        }
                    }
                }
            }
        });

        let output = match tokio::time::timeout(
            tokio::time::Duration::from_secs(900),
            scan_task,
        ).await {
            Ok(Ok(Ok(o))) => {
                tracing::info!("Scan {} remote script returned {} bytes", scan_id, o.len());
                o
            }
            Ok(Ok(Err(e))) => {
                tracing::error!("Scan {} host {} scan failed: {}", scan_id, host.name, e);
                let _ = db::update_host_status(&state.db, host.id, "error").await;
                let _ = db::fail_scan(&state.db, scan_id, &format!("扫描失败: {}", e)).await;
                continue;
            }
            Ok(Err(e)) => {
                tracing::error!("Scan {} host {} scan task join error: {}", scan_id, host.name, e);
                let _ = db::fail_scan(&state.db, scan_id, "扫描任务异常").await;
                continue;
            }
            Err(_) => {
                tracing::error!("Scan {} host {} scan timed out after 15 minutes", scan_id, host.name);
                let _ = db::update_host_status(&state.db, host.id, "error").await;
                let _ = db::fail_scan(&state.db, scan_id, "扫描超时（15分钟）").await;
                continue;
            }
        };

        // 解析 JSON 输出（尝试修复截断的JSON）
        let preview_len = output.len().min(200);
        tracing::info!("Scan {} parsing script output ({} bytes): {}...", scan_id, output.len(), &output[..preview_len]);

        // 尝试修复可能被截断的JSON
        let fixed_output = if output.contains("\"results\":[") && !output.trim().ends_with("]}") {
            // 找到最后一个完整的JSON对象，补上闭合括号
            if let Some(last_brace) = output.rfind('}') {
                let truncated = &output[..last_brace + 1];
                format!("{}]}}", truncated)
            } else {
                output.clone()
            }
        } else {
            output.clone()
        };

        let script_output: ScriptOutput = match serde_json::from_str(&fixed_output) {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Scan {} host {} output parse failed: {}, raw: {}", scan_id, host.name, e, output);
                let _ = db::fail_scan(&state.db, scan_id, &format!("脚本输出解析失败: {}", e)).await;
                continue;
            }
        };
        tracing::info!("Scan {} parsed {} VM results", scan_id, script_output.results.len());

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
            // 检查是否被用户停止
            let scan_status: String = sqlx::query_scalar("SELECT status FROM scans WHERE id = ?")
                .bind(scan_id)
                .fetch_one(&state.db)
                .await
                .unwrap_or_default();
            if scan_status == "stopped" {
                tracing::info!("Scan {} stopped by user at VM {}/{}", scan_id, i, total);
                break;
            }

            let evidence = r.evidence.as_deref().unwrap_or("");

            // 统计检测方式（所有 VM 都计入）
            match r.method.as_str() {
                "ga" => ga_count += 1,
                "disk" => disk_count += 1,
                _ => {}
            }

            // 跳过停止的 VM（不入库）
            if evidence == "vm_stopped" {
                continue;
            }

            let status = if r.status == "detected" {
                found_count += 1;
                current_detected.insert(r.vmid.clone());
                if prev_detected.contains(&r.vmid) { "confirmed" } else { "detected" }
            } else if r.status == "clean" {
                if prev_detected.contains(&r.vmid) { "cleaned" } else { "clean" }
            } else if r.status == "skipped" {
                // 停止的VM跳过（批量模式），不入库不统计
                continue;
            } else if r.status == "needs_disk_scan" {
                "needs_disk_scan"
            } else if r.status == "error" {
                "error"
            } else {
                "unknown"
            };

            // 每台 VM 都落库（覆盖历史状态行，避免残留垃圾数据）
            db::upsert_result(&state.db, scan_id, host.id, &r.vmid, status, &r.method, evidence).await?;

            if status == "detected" || status == "cleaned" {
                let ev: Vec<String> = evidence.split_whitespace().map(|s| s.to_string()).collect();
                crate::notify::send_all(&state.db, &state.master_key, status, &host.name, &r.vmid, &ev.join(" ")).await;
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
