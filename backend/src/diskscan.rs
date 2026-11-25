//! 磁盘扫描后台队列
//! - 单台 / 批量 / 定时自动 三种来源统一入队，由后端执行；网页切走或关闭不影响
//! - 每台宿主机只有一个 /dev/nbd1：同一宿主机串行，不同宿主机并行
//! - 白名单：磁盘扫描连续 N 次干净自动加入，加入 M 天后失效；命中立即移出；
//!   批量与定时自动扫描跳过白名单，手动单台扫描不跳过
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::SqlitePool;
use crate::{AppState, db};

const KEY_CLEAN_COUNT: &str = "disk_whitelist_clean_count";
const KEY_DAYS: &str = "disk_whitelist_days";
const DEFAULT_CLEAN_COUNT: i64 = 3;
const DEFAULT_DAYS: i64 = 30;

#[derive(Default)]
pub struct DiskScanManager {
    /// 正在运行 worker 的宿主机（每台宿主机最多一个 worker）
    active_hosts: Mutex<HashSet<i64>>,
    /// 入队互斥：防止并发入队时「查重」与「插入」交错导致同一 VM 重复排队
    enqueue_lock: tokio::sync::Mutex<()>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DiskScanJob {
    pub id: i64,
    pub source: String,
    pub scan_id: Option<i64>,
    pub status: String,
    pub total: i64,
    pub done: i64,
    pub found: i64,
    pub failed: i64,
    pub skipped: i64,
    pub error: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DiskScanTask {
    pub id: i64,
    pub job_id: i64,
    pub host_id: i64,
    pub vmid: String,
    pub status: String,
    pub detail: Option<String>,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WhitelistEntry {
    pub host_id: i64,
    pub host_name: Option<String>,
    pub vmid: String,
    pub clean_streak: i64,
    pub source: Option<String>,
    pub whitelisted_at: Option<NaiveDateTime>,
    pub expires_at: Option<NaiveDateTime>,
    pub last_scan_at: Option<NaiveDateTime>,
}

/// 结果列表附带的白名单信息
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WhitelistInfo {
    pub host_id: i64,
    pub vmid: String,
    pub clean_streak: i64,
    pub source: Option<String>,
    pub active: bool,
    pub expires_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize)]
pub struct EnqueueOutcome {
    pub job_id: Option<i64>,
    pub queued: usize,
    /// 在白名单内而跳过
    pub whitelisted: usize,
    /// 已在队列中（排队或扫描中）而跳过
    pub duplicated: usize,
}

pub fn is_valid_vmid(v: &str) -> bool {
    !v.is_empty() && v.len() <= 10 && v.chars().all(|c| c.is_ascii_digit())
}

fn days_modifier(days: i64) -> String {
    format!("+{} days", days)
}

// ---------------- 设置 ----------------

async fn setting_i64(pool: &SqlitePool, key: &str, default: i64) -> i64 {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// (自动加入白名单所需的连续干净次数, 白名单有效天数)
pub async fn get_settings(pool: &SqlitePool) -> (i64, i64) {
    (
        setting_i64(pool, KEY_CLEAN_COUNT, DEFAULT_CLEAN_COUNT).await,
        setting_i64(pool, KEY_DAYS, DEFAULT_DAYS).await,
    )
}

pub async fn save_settings(pool: &SqlitePool, clean_count: i64, days: i64) -> anyhow::Result<()> {
    for (k, v) in [(KEY_CLEAN_COUNT, clean_count), (KEY_DAYS, days)] {
        sqlx::query("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP")
            .bind(k).bind(v.to_string()).execute(pool).await?;
    }
    Ok(())
}

// ---------------- 白名单 ----------------

/// 当前有效（未过期）的白名单
async fn active_whitelist(pool: &SqlitePool, days: i64) -> HashSet<(i64, String)> {
    sqlx::query_as::<_, (i64, String)>(
        "SELECT host_id, vmid FROM disk_scan_whitelist
         WHERE whitelisted_at IS NOT NULL AND datetime(whitelisted_at, ?) > datetime('now')")
        .bind(days_modifier(days))
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect()
}

/// 有效白名单列表（设置页展示）
pub async fn list_whitelist(pool: &SqlitePool) -> anyhow::Result<Vec<WhitelistEntry>> {
    let (_, days) = get_settings(pool).await;
    let m = days_modifier(days);
    Ok(sqlx::query_as::<_, WhitelistEntry>(
        "SELECT w.host_id, h.name AS host_name, w.vmid, w.clean_streak, w.source, w.whitelisted_at,
                datetime(w.whitelisted_at, ?) AS expires_at, w.last_scan_at
         FROM disk_scan_whitelist w JOIN hosts h ON h.id = w.host_id
         WHERE w.whitelisted_at IS NOT NULL AND datetime(w.whitelisted_at, ?) > datetime('now')
         ORDER BY w.whitelisted_at DESC")
        .bind(&m).bind(&m).fetch_all(pool).await?)
}

/// 全部白名单/累计记录，按 (host_id, vmid) 索引（结果列表附带展示用）
pub async fn whitelist_map(pool: &SqlitePool) -> HashMap<(i64, String), WhitelistInfo> {
    let (_, days) = get_settings(pool).await;
    let m = days_modifier(days);
    sqlx::query_as::<_, WhitelistInfo>(
        "SELECT host_id, vmid, clean_streak, source,
                (whitelisted_at IS NOT NULL AND datetime(whitelisted_at, ?) > datetime('now')) AS active,
                datetime(whitelisted_at, ?) AS expires_at
         FROM disk_scan_whitelist")
        .bind(&m).bind(&m)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|w| ((w.host_id, w.vmid.clone()), w))
        .collect()
}

/// 手动加入白名单（已加入的刷新加入时间，相当于续期）
pub async fn whitelist_add(pool: &SqlitePool, host_id: i64, vmid: &str) -> anyhow::Result<()> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM results WHERE host_id = ? AND vmid = ? ORDER BY id DESC LIMIT 1")
        .bind(host_id).bind(vmid).fetch_optional(pool).await?;
    if matches!(status.as_deref(), Some("detected") | Some("confirmed")) {
        return Err(anyhow::anyhow!("该 VM 当前为命中状态，不能加入白名单"));
    }
    sqlx::query("INSERT INTO disk_scan_whitelist (host_id, vmid, clean_streak, whitelisted_at, source, updated_at)
                 VALUES (?, ?, 0, CURRENT_TIMESTAMP, 'manual', CURRENT_TIMESTAMP)
                 ON CONFLICT(host_id, vmid) DO UPDATE SET whitelisted_at = CURRENT_TIMESTAMP, source = 'manual', updated_at = CURRENT_TIMESTAMP")
        .bind(host_id).bind(vmid).execute(pool).await?;
    Ok(())
}

/// 移出白名单并清零连续干净次数（手动移出 / 检测命中）
pub async fn whitelist_remove(pool: &SqlitePool, host_id: i64, vmid: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM disk_scan_whitelist WHERE host_id = ? AND vmid = ?")
        .bind(host_id).bind(vmid).execute(pool).await?;
    Ok(())
}

/// 磁盘扫描干净：连续干净次数 +1，达到阈值自动加入白名单；已失效的白名单先复位重新累计
async fn record_clean(pool: &SqlitePool, host_id: i64, vmid: &str) {
    let (threshold, days) = get_settings(pool).await;
    let _ = sqlx::query(
        "UPDATE disk_scan_whitelist SET clean_streak = 0, whitelisted_at = NULL, source = NULL
         WHERE host_id = ? AND vmid = ? AND whitelisted_at IS NOT NULL AND datetime(whitelisted_at, ?) <= datetime('now')")
        .bind(host_id).bind(vmid).bind(days_modifier(days)).execute(pool).await;
    let _ = sqlx::query(
        "INSERT INTO disk_scan_whitelist (host_id, vmid, clean_streak, last_scan_at, updated_at)
         VALUES (?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
         ON CONFLICT(host_id, vmid) DO UPDATE SET clean_streak = clean_streak + 1,
             last_scan_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP")
        .bind(host_id).bind(vmid).execute(pool).await;
    // 阈值 0 = 不自动加入
    if threshold > 0 {
        let r = sqlx::query(
            "UPDATE disk_scan_whitelist SET whitelisted_at = CURRENT_TIMESTAMP, source = 'auto', updated_at = CURRENT_TIMESTAMP
             WHERE host_id = ? AND vmid = ? AND whitelisted_at IS NULL AND clean_streak >= ?")
            .bind(host_id).bind(vmid).bind(threshold).execute(pool).await;
        if matches!(r, Ok(ref x) if x.rows_affected() > 0) {
            tracing::info!("Host {} VM {} 磁盘扫描连续 {} 次干净，自动加入白名单", host_id, vmid, threshold);
        }
    }
}

// ---------------- 任务 ----------------

/// 服务启动时：上次未完成的任务标记为中断（不自动续跑，需要的话手动重新发起）
pub async fn mark_interrupted(pool: &SqlitePool) {
    // 先记下受影响的任务（含已停止但还有一台在扫的），改完后统一重算计数
    let job_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT job_id FROM disk_scan_tasks WHERE status IN ('pending', 'running')")
        .fetch_all(pool).await.unwrap_or_default();
    let _ = sqlx::query("UPDATE disk_scan_tasks SET status = 'cancelled', detail = '服务重启中断', completed_at = CURRENT_TIMESTAMP
                         WHERE status IN ('pending', 'running')").execute(pool).await;
    let r = sqlx::query("UPDATE disk_scan_jobs SET status = 'interrupted', error = '服务重启中断', completed_at = CURRENT_TIMESTAMP
                         WHERE status IN ('queued', 'running')").execute(pool).await;
    if let Ok(r) = r {
        if r.rows_affected() > 0 {
            tracing::warn!("{} 个磁盘扫描任务因服务重启中断", r.rows_affected());
        }
    }
    for id in job_ids {
        refresh_job(pool, id).await;
    }
}

/// 重算任务计数；全部 VM 有结果后标记完成（已停止/中断的不改状态）
async fn refresh_job(pool: &SqlitePool, job_id: i64) {
    let _ = sqlx::query(
        "UPDATE disk_scan_jobs SET
            done = (SELECT COUNT(*) FROM disk_scan_tasks WHERE job_id = ? AND status IN ('clean', 'detected', 'error')),
            found = (SELECT COUNT(*) FROM disk_scan_tasks WHERE job_id = ? AND status = 'detected'),
            failed = (SELECT COUNT(*) FROM disk_scan_tasks WHERE job_id = ? AND status = 'error'),
            skipped = (SELECT COUNT(*) FROM disk_scan_tasks WHERE job_id = ? AND status = 'cancelled')
         WHERE id = ?")
        .bind(job_id).bind(job_id).bind(job_id).bind(job_id).bind(job_id)
        .execute(pool).await;
    let _ = sqlx::query(
        "UPDATE disk_scan_jobs SET status = 'completed', completed_at = CURRENT_TIMESTAMP
         WHERE id = ? AND status IN ('queued', 'running')
           AND NOT EXISTS (SELECT 1 FROM disk_scan_tasks WHERE job_id = ? AND status IN ('pending', 'running'))")
        .bind(job_id).bind(job_id)
        .execute(pool).await;
}

/// 入队。skip_whitelisted=true 时跳过有效白名单（批量/定时）；已在队列中的 VM 不重复排队
pub async fn enqueue(
    state: &Arc<AppState>,
    source: &str,
    scan_id: Option<i64>,
    targets: Vec<(i64, String)>,
    skip_whitelisted: bool,
) -> anyhow::Result<EnqueueOutcome> {
    let pool = &state.db;
    let _guard = state.disk_scan.enqueue_lock.lock().await;

    let mut seen = HashSet::new();
    let targets: Vec<(i64, String)> = targets
        .into_iter()
        .filter(|(_, v)| is_valid_vmid(v))
        .filter(|t| seen.insert(t.clone()))
        .collect();

    let wl = if skip_whitelisted {
        let (_, days) = get_settings(pool).await;
        active_whitelist(pool, days).await
    } else {
        HashSet::new()
    };
    let busy: HashSet<(i64, String)> = sqlx::query_as::<_, (i64, String)>(
        "SELECT host_id, vmid FROM disk_scan_tasks WHERE status IN ('pending', 'running')")
        .fetch_all(pool).await?
        .into_iter()
        .collect();

    let mut out = EnqueueOutcome { job_id: None, queued: 0, whitelisted: 0, duplicated: 0 };
    let mut list = Vec::new();
    for t in targets {
        if wl.contains(&t) {
            out.whitelisted += 1;
        } else if busy.contains(&t) {
            out.duplicated += 1;
        } else {
            list.push(t);
        }
    }
    if list.is_empty() {
        return Ok(out);
    }

    let mut tx = pool.begin().await?;
    let job_id = sqlx::query("INSERT INTO disk_scan_jobs (source, scan_id, total) VALUES (?, ?, ?)")
        .bind(source).bind(scan_id).bind(list.len() as i64)
        .execute(&mut *tx).await?
        .last_insert_rowid();
    for (host_id, vmid) in &list {
        sqlx::query("INSERT INTO disk_scan_tasks (job_id, host_id, vmid) VALUES (?, ?, ?)")
            .bind(job_id).bind(host_id).bind(vmid)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;

    out.job_id = Some(job_id);
    out.queued = list.len();
    tracing::info!("Disk scan job {} ({}) queued {} VMs", job_id, source, list.len());

    let hosts: HashSet<i64> = list.iter().map(|(h, _)| *h).collect();
    for h in hosts {
        kick(state, h);
    }
    Ok(out)
}

/// 停止任务：未开始的 VM 取消；正在扫描的那台会扫完（远端有超时兜底）
pub async fn stop_job(pool: &SqlitePool, job_id: i64) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let n = sqlx::query("UPDATE disk_scan_jobs SET status = 'stopped', completed_at = CURRENT_TIMESTAMP
                         WHERE id = ? AND status IN ('queued', 'running')")
        .bind(job_id).execute(&mut *tx).await?.rows_affected();
    sqlx::query("UPDATE disk_scan_tasks SET status = 'cancelled', detail = '已停止', completed_at = CURRENT_TIMESTAMP
                 WHERE job_id = ? AND status = 'pending'")
        .bind(job_id).execute(&mut *tx).await?;
    tx.commit().await?;
    refresh_job(pool, job_id).await;
    Ok(n > 0)
}

/// 定时任务勾选了「自动磁盘扫描」：常规扫描结束后把本次「待检测」的 VM 入队（跳过白名单）
pub async fn enqueue_after_scan(state: &Arc<AppState>, scan_id: i64, outcome: crate::detect::ScanOutcome) {
    if outcome.stopped {
        tracing::info!("Scan {} 被手动停止，不触发自动磁盘扫描", scan_id);
        return;
    }
    if outcome.needs_disk.is_empty() {
        return;
    }
    match enqueue(state, "schedule", Some(scan_id), outcome.needs_disk, true).await {
        Ok(o) => tracing::info!(
            "Scan {} 自动磁盘扫描：入队 {} 台，白名单跳过 {} 台，已在队列 {} 台",
            scan_id, o.queued, o.whitelisted, o.duplicated
        ),
        Err(e) => tracing::error!("Scan {} 自动磁盘扫描入队失败: {}", scan_id, e),
    }
}

/// 确保该宿主机有 worker 在跑
fn kick(state: &Arc<AppState>, host_id: i64) {
    if !lock_hosts(state).insert(host_id) {
        return; // 已有 worker，会按顺序取到新任务
    }
    let state = state.clone();
    tokio::spawn(async move {
        let mut slot = HostSlot { state: state.clone(), host_id };
        loop {
            let drained = host_worker(&state, host_id).await;
            drop(slot);
            if !drained {
                break; // 数据库异常退出，不空转；剩余任务等下次入队时再拉起
            }
            // 退出前再查一次：防止「最后一次取任务」到「移出活跃集合」之间新入队的任务没人执行
            let pending: Result<i64, _> = sqlx::query_scalar(
                "SELECT COUNT(*) FROM disk_scan_tasks WHERE host_id = ? AND status = 'pending'")
                .bind(host_id).fetch_one(&state.db).await;
            match pending {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Disk scan worker host {} recheck failed: {}", host_id, e);
                    break;
                }
            }
            if !lock_hosts(&state).insert(host_id) {
                break;
            }
            slot = HostSlot { state: state.clone(), host_id };
        }
    });
}

/// worker 占位：离开作用域（含 panic）时把宿主机移出活跃集合，避免该宿主机永远拉不起 worker
struct HostSlot {
    state: Arc<AppState>,
    host_id: i64,
}

impl Drop for HostSlot {
    fn drop(&mut self) {
        lock_hosts(&self.state).remove(&self.host_id);
    }
}

fn lock_hosts(state: &AppState) -> std::sync::MutexGuard<'_, HashSet<i64>> {
    state.disk_scan.active_hosts.lock().unwrap_or_else(|e| e.into_inner())
}

/// 逐个执行该宿主机的待扫描 VM（先进先出）。队列取空返回 true，数据库异常返回 false
async fn host_worker(state: &Arc<AppState>, host_id: i64) -> bool {
    loop {
        let next = sqlx::query_as::<_, (i64, i64, String)>(
            "SELECT id, job_id, vmid FROM disk_scan_tasks WHERE host_id = ? AND status = 'pending' ORDER BY id LIMIT 1")
            .bind(host_id).fetch_optional(&state.db).await;
        let (task_id, job_id, vmid) = match next {
            Ok(Some(t)) => t,
            Ok(None) => return true,
            Err(e) => {
                tracing::error!("Disk scan worker host {} query failed: {}", host_id, e);
                return false;
            }
        };
        // 认领：期间被停止（已取消）则跳过
        let claimed = sqlx::query("UPDATE disk_scan_tasks SET status = 'running', started_at = CURRENT_TIMESTAMP
                                   WHERE id = ? AND status = 'pending'")
            .bind(task_id).execute(&state.db).await;
        match claimed {
            Ok(r) if r.rows_affected() == 1 => {}
            Ok(_) => continue,
            Err(e) => {
                tracing::error!("Disk scan worker host {} claim failed: {}", host_id, e);
                return false;
            }
        }
        let _ = sqlx::query("UPDATE disk_scan_jobs SET status = 'running', started_at = COALESCE(started_at, CURRENT_TIMESTAMP)
                             WHERE id = ? AND status = 'queued'")
            .bind(job_id).execute(&state.db).await;

        let (status, detail) = run_one(state, host_id, &vmid).await;
        tracing::info!("Disk scan job {} host {} VM {}: {} {}", job_id, host_id, vmid, status, detail);

        let _ = sqlx::query("UPDATE disk_scan_tasks SET status = ?, detail = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(status).bind(&detail).bind(task_id).execute(&state.db).await;
        refresh_job(&state.db, job_id).await;
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 脚本错误代码 → 中文说明
fn error_text(code: &str) -> String {
    match code {
        "no_disk_config" => "未找到磁盘配置".to_string(),
        "path_not_found" => "磁盘路径解析失败".to_string(),
        "nbd_failed" => "磁盘连接失败（qemu-nbd）".to_string(),
        "mount_failed" => "无法挂载文件系统（加密/LVM/不支持的文件系统等）".to_string(),
        "disk_busy" => "宿主机上另一个磁盘扫描占用中，等待超时".to_string(),
        "" => "检测失败".to_string(),
        other => truncate(other, 200),
    }
}

/// 扫描一台 VM 并落库，返回任务状态（clean / detected / error）与说明
async fn run_one(state: &Arc<AppState>, host_id: i64, vmid: &str) -> (&'static str, String) {
    let host = match db::get_host(&state.db, host_id).await {
        Ok(h) => h,
        Err(_) => return ("error", "主机不存在".to_string()),
    };
    let auth = crate::deploy::SshAuth::from_host(
        host.password_encrypted.as_deref(),
        host.ssh_key_encrypted.as_deref(),
        &state.master_key,
    );
    let output = match crate::deploy::run_remote_disk_scan(&host.host, host.port as u16, &host.username, &auth, vmid).await {
        Ok(o) => o,
        Err(e) => return ("error", truncate(&e.to_string(), 200)),
    };

    let item = serde_json::from_str::<serde_json::Value>(output.trim())
        .ok()
        .and_then(|v| {
            v.get("results")?
                .as_array()?
                .iter()
                .find(|r| r.get("vmid").and_then(|x| x.as_str()) == Some(vmid))
                .cloned()
        });
    let Some(item) = item else {
        tracing::warn!("Disk scan {} VM {} unparsable output: {}", host.name, vmid, truncate(&output, 300));
        return ("error", "脚本输出不完整（可能超时）".to_string());
    };
    let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let method = item.get("method").and_then(|v| v.as_str()).unwrap_or("disk");
    let evidence = item.get("evidence").and_then(|v| v.as_str()).unwrap_or("");

    match status {
        "detected" | "clean" => {
            if let Err(e) = apply_disk_result(state, &host, vmid, status, method, evidence).await {
                return ("error", format!("结果写入失败: {}", truncate(&e.to_string(), 150)));
            }
            if status == "detected" {
                ("detected", evidence.split("##").next().unwrap_or("").trim().to_string())
            } else {
                ("clean", String::new())
            }
        }
        // 扫描失败不改检测结果（保持原状态），也不影响白名单累计
        _ => ("error", error_text(evidence)),
    }
}

/// 磁盘扫描结果落库：状态变迁留档 + 通知 + 白名单
async fn apply_disk_result(
    state: &Arc<AppState>,
    host: &db::Host,
    vmid: &str,
    status: &str,
    method: &str,
    evidence: &str,
) -> anyhow::Result<()> {
    let pool = &state.db;
    let prev = sqlx::query_as::<_, db::Result>(
        "SELECT * FROM results WHERE host_id = ? AND vmid = ? ORDER BY id DESC LIMIT 1")
        .bind(host.id).bind(vmid).fetch_optional(pool).await?;
    let was_bad = prev.as_ref().map_or(false, |p| p.status == "detected" || p.status == "confirmed");

    if status == "detected" {
        let st = if was_bad { "confirmed" } else { "detected" };
        db::upsert_result(pool, 0, host.id, vmid, st, method, evidence).await?;
        let _ = whitelist_remove(pool, host.id, vmid).await;
        if !was_bad {
            let _ = db::insert_history(pool, host.id, vmid, "detected", method, evidence, None).await;
            // 通知里去掉 ## 明细段，只发证据代码（与常规扫描一致）
            let ev_head = evidence.split("##").next().unwrap_or(evidence);
            let ev = ev_head.split_whitespace().collect::<Vec<_>>().join(" ");
            crate::notify::send_all(pool, &state.master_key, "detected", &host.name, vmid, &ev).await;
        }
    } else {
        if was_bad {
            // 之前命中、现在干净 → 已清除（保留原证据供回看）
            let prev_ev = prev.as_ref().and_then(|p| p.evidence.clone()).unwrap_or_default();
            db::upsert_result(pool, 0, host.id, vmid, "cleaned", method, &prev_ev).await?;
            let _ = db::insert_history(pool, host.id, vmid, "cleaned", method, &prev_ev, None).await;
            crate::notify::send_all(pool, &state.master_key, "cleaned", &host.name, vmid, "切鸡软件已清除").await;
        } else {
            db::upsert_result(pool, 0, host.id, vmid, "clean", method, evidence).await?;
        }
        record_clean(pool, host.id, vmid).await;
    }
    Ok(())
}

// ---------------- 查询 ----------------

pub async fn get_job(pool: &SqlitePool, id: i64) -> anyhow::Result<(DiskScanJob, Vec<DiskScanTask>)> {
    let job = sqlx::query_as::<_, DiskScanJob>("SELECT * FROM disk_scan_jobs WHERE id = ?")
        .bind(id).fetch_one(pool).await?;
    let tasks = sqlx::query_as::<_, DiskScanTask>("SELECT * FROM disk_scan_tasks WHERE job_id = ? ORDER BY id")
        .bind(id).fetch_all(pool).await?;
    Ok((job, tasks))
}

/// 进行中的任务、排队/扫描中的 VM、最近一次结束的任务，以及待检测 VM 计数
pub async fn active_overview(pool: &SqlitePool) -> anyhow::Result<serde_json::Value> {
    let jobs = sqlx::query_as::<_, DiskScanJob>(
        "SELECT * FROM disk_scan_jobs WHERE status IN ('queued', 'running') ORDER BY id")
        .fetch_all(pool).await?;
    let tasks = sqlx::query_as::<_, (i64, i64, String, String)>(
        "SELECT job_id, host_id, vmid, status FROM disk_scan_tasks WHERE status IN ('pending', 'running') ORDER BY id")
        .fetch_all(pool).await?;
    let last = sqlx::query_as::<_, DiskScanJob>(
        "SELECT * FROM disk_scan_jobs WHERE status NOT IN ('queued', 'running') ORDER BY id DESC LIMIT 1")
        .fetch_optional(pool).await?;

    let (_, days) = get_settings(pool).await;
    let m = days_modifier(days);
    let (needs_total, needs_whitelisted): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*),
                COALESCE(SUM(EXISTS (SELECT 1 FROM disk_scan_whitelist w
                     WHERE w.host_id = r.host_id AND w.vmid = r.vmid
                       AND w.whitelisted_at IS NOT NULL AND datetime(w.whitelisted_at, ?) > datetime('now'))), 0)
         FROM results r WHERE r.status = 'needs_disk_scan'")
        .bind(&m).fetch_one(pool).await?;

    let tasks: Vec<serde_json::Value> = tasks
        .into_iter()
        .map(|(job_id, host_id, vmid, status)| serde_json::json!({
            "job_id": job_id, "host_id": host_id, "vmid": vmid, "status": status,
        }))
        .collect();
    Ok(serde_json::json!({
        "jobs": jobs,
        "tasks": tasks,
        "last": last,
        "needs_total": needs_total,
        "needs_whitelisted": needs_whitelisted,
    }))
}

/// 批量扫描目标：当前所有「待检测」的 VM
pub async fn needs_disk_targets(pool: &SqlitePool) -> anyhow::Result<Vec<(i64, String)>> {
    Ok(sqlx::query_as::<_, (i64, String)>(
        "SELECT host_id, vmid FROM results WHERE status = 'needs_disk_scan' AND host_id IS NOT NULL ORDER BY host_id, id")
        .fetch_all(pool).await?)
}
