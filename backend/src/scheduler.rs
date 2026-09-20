use std::sync::Arc;
use std::str::FromStr;
use chrono::Local;
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::AppState;

/// 规范化 cron 表达式：5 段（分 时 日 月 周）自动补秒位，6/7 段原样
fn normalize_cron(expr: &str) -> String {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() == 5 {
        format!("0 {}", expr.trim())
    } else {
        expr.trim().to_string()
    }
}

/// 计算下次执行时间（本地时区）。表达式非法返回 None。
pub fn compute_next_run(cron_expr: &str) -> Option<chrono::NaiveDateTime> {
    let sched = cron::Schedule::from_str(&normalize_cron(cron_expr)).ok()?;
    sched.upcoming(Local).next().map(|d| d.naive_local())
}

pub fn is_valid_cron(cron_expr: &str) -> bool {
    cron::Schedule::from_str(&normalize_cron(cron_expr)).is_ok()
}

pub async fn start(state: Arc<AppState>) -> anyhow::Result<()> {
    let mut sched = JobScheduler::new().await?;

    // Check schedules every minute
    let state_clone = state.clone();
    let job = Job::new("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        tokio::spawn(async move {
            if let Err(e) = check_and_run_schedules(state).await {
                tracing::error!("Scheduler check failed: {}", e);
            }
        });
    })?;

    sched.add(job).await?;
    sched.start().await?;

    tracing::info!("Scheduler started");
    Ok(())
}

async fn check_and_run_schedules(state: Arc<AppState>) -> anyhow::Result<()> {
    let schedules = crate::db::list_schedules(&state.db).await?;
    let now = Local::now().naive_local();

    for schedule in &schedules {
        if !schedule.enabled { continue; }

        // next_run 缺失（新建/老数据）先补算，不立即触发
        let next = match schedule.next_run {
            Some(n) => n,
            None => match compute_next_run(&schedule.cron_expr) {
                Some(n) => {
                    let _ = crate::db::set_schedule_next_run(&state.db, schedule.id, Some(n)).await;
                    n
                }
                None => {
                    tracing::warn!("Schedule {} has invalid cron: {}", schedule.id, schedule.cron_expr);
                    continue;
                }
            },
        };

        if next <= now {
            tracing::info!("Running scheduled scan {} (cron={:?}, host={:?})", schedule.id, schedule.cron_expr, schedule.host_id);
            let scan = crate::db::create_scan(&state.db, schedule.host_id).await?;
            let state_clone = state.clone();
            let scan_id = scan.id;
            let host_id = schedule.host_id;

            tokio::spawn(async move {
                if let Err(e) = crate::detect::run_scan(state_clone, scan_id, host_id).await {
                    tracing::error!("Scheduled scan {} failed: {}", scan_id, e);
                }
            });

            // 记录执行并预排下一次
            let new_next = compute_next_run(&schedule.cron_expr);
            crate::db::update_schedule_run(&state.db, schedule.id, new_next).await?;
        }
    }

    Ok(())
}
