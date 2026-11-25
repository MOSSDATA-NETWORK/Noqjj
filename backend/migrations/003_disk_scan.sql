-- 磁盘扫描后台任务：单台 / 批量 / 定时自动，统一由后端队列执行（切走页面不中断）
CREATE TABLE IF NOT EXISTS disk_scan_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,                     -- single / batch / schedule
    scan_id INTEGER,                          -- 定时自动触发时关联的常规扫描
    status TEXT NOT NULL DEFAULT 'queued',    -- queued / running / completed / stopped / interrupted
    total INTEGER NOT NULL DEFAULT 0,
    done INTEGER NOT NULL DEFAULT 0,          -- 已出结果（干净/命中/失败）
    found INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    skipped INTEGER NOT NULL DEFAULT 0,       -- 停止或服务重启而取消
    error TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME
);

-- 任务内每台 VM 一行
CREATE TABLE IF NOT EXISTS disk_scan_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    host_id INTEGER NOT NULL,
    vmid TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',   -- pending / running / clean / detected / error / cancelled
    detail TEXT,
    started_at DATETIME,
    completed_at DATETIME
);
CREATE INDEX IF NOT EXISTS idx_disk_tasks_job ON disk_scan_tasks(job_id, status);
CREATE INDEX IF NOT EXISTS idx_disk_tasks_host ON disk_scan_tasks(host_id, status);

-- 磁盘扫描白名单：连续干净次数 + 加入时间；失效时间 = whitelisted_at + 设置里的天数（动态计算，改设置即时生效）
CREATE TABLE IF NOT EXISTS disk_scan_whitelist (
    host_id INTEGER NOT NULL,
    vmid TEXT NOT NULL,
    clean_streak INTEGER NOT NULL DEFAULT 0,
    whitelisted_at DATETIME,
    source TEXT,                              -- auto / manual
    last_scan_at DATETIME,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (host_id, vmid)
);
