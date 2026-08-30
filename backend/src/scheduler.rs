use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::AppState;

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

    for schedule in &schedules {
        if !schedule.enabled { continue; }

        let should_run = match &schedule.last_run {
            Some(last) => {
                let elapsed = chrono::Utc::now().naive_utc() - *last;
                elapsed.num_minutes() >= 60 // Minimum 1 hour between runs
            }
            None => true,
        };

        if should_run {
            tracing::info!("Running scheduled scan for host {:?}", schedule.host_id);
            let scan = crate::db::create_scan(&state.db, schedule.host_id).await?;
            let state_clone = state.clone();
            let scan_id = scan.id;
            let host_id = schedule.host_id;

            tokio::spawn(async move {
                if let Err(e) = crate::detect::run_scan(state_clone, scan_id, host_id).await {
                    tracing::error!("Scheduled scan {} failed: {}", scan_id, e);
                }
            });

            // Update last_run
            sqlx::query("UPDATE schedules SET last_run=CURRENT_TIMESTAMP WHERE id=?")
                .bind(schedule.id)
                .execute(&state.db)
                .await?;
        }
    }

    Ok(())
}
