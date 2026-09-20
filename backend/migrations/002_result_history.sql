-- 检测历史：每次状态变迁（新发现/已清除/再次发现）留档，供详情页回看
CREATE TABLE IF NOT EXISTS result_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id INTEGER NOT NULL,
    vmid TEXT NOT NULL,
    status TEXT NOT NULL,
    method TEXT,
    evidence TEXT,
    scan_id INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_history_vm ON result_history(host_id, vmid);
