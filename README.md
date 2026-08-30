<p align="center">
  <img src="https://img.shields.io/badge/Noqjj-禁止切鸡鸡-red?style=for-the-badge" alt="Noqjj">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Vue.js-35495E?style=for-the-badge&logo=vuedotjs&logoColor=4FC08D" alt="Vue.js">
  <img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="License">
</p>

<h1 align="center">Noqjj — 禁止切鸡鸡</h1>

<p align="center">
  <b>PVE 切鸡检测平台</b><br>
  自动扫描虚拟机是否安装了 Incus / NodeHatch / shlii.io 等切鸡软件，发现即告警。
</p>

<p align="center">
  <a href="#中文">中文</a> •
  <a href="#english">English</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#部署">部署</a> •
  <a href="#api">API</a>
</p>

---

## 中文

### 这是什么

**Noqjj**（禁止切鸡鸡）是一个 PVE 切鸡检测平台。它可以：

- 🔍 **自动检测** — 扫描 PVE 宿主机上的所有 VM，识别是否安装了切鸡软件
- 🚀 **自动部署** — 添加 PVE 主机后，自动上传检测脚本，无需手动操作
- 🔐 **安全认证** — 支持密码 + TOTP 身份验证器，数据 AES-256-GCM 加密
- 🔑 **多种接入** — SSH 密码 / SSH Key / PVE API Token
- 📱 **实时告警** — Telegram + 企业微信，新发现或已清除即时通知
- 🎨 **Apple Design** — 简洁优雅的 UI

### 什么是"切鸡"

"切鸡"是 IDC 行业术语，指在一台 VPS 内部再安装虚拟化软件（如 Incus/LXD），把一台 VPS 拆成多份二次售卖。这对 IDC 运营商来说是违规行为，会导致资源超卖、性能下降、安全隐患等问题。

### 检测原理

| 方式 | 条件 | 检测内容 |
|------|------|---------|
| **GA 模式** | VM 安装了 qemu-guest-agent | 文件、systemd 服务、bash_history、网络连接 |
| **磁盘挂载** | GA 不可用 | 复制磁盘 → qemu-nbd 只读挂载 → 检查文件系统 |

### 检测特征

| 类型 | 路径 / 名称 |
|------|-------------|
| Incus 二进制 | `/opt/incus/` |
| shlii.io agent | `/usr/local/bin/incushlii-agent` |
| NodeHatch agent | `/usr/local/bin/nodeget-agent` |
| systemd 服务 | `incushlii-agent.service`、`nodeget-agent.service` 等 |
| bash_history | `shlii.io`、`nodeget`、`incushlii` 关键词 |

### 快速开始

```bash
# 1. 克隆
git clone https://github.com/MOSSDATA-NETWORK/Noqjj.git
cd Noqjj

# 2. 编译前端
cd frontend && npm install && npm run build && cd ..

# 3. 编译后端
cd backend && cargo build --release && cd ..

# 4. 部署
scp backend/target/release/chicken-detect-backend 你的服务器:/opt/noqjj/
scp -r backend/static 你的服务器:/opt/noqjj/

# 5. 运行
ssh 你的服务器
cd /opt/noqjj
chmod +x chicken-detect-backend
./chicken-detect-backend

# 6. 浏览器访问 http://你的服务器IP:3210
```

### 使用流程

```
首次访问 → /setup 设置管理员账户（支持 TOTP）
    ↓
  控制台 → 添加 PVE 主机（选择接入方式）
    ↓
  自动部署 → 检测脚本上传到 PVE
    ↓
  扫描 → 查看结果 → 配置通知 → 定时巡检
```

### PVE 接入方式

| 方式 | 说明 | 适用场景 |
|------|------|---------|
| SSH 密码 | 最常用 | 默认方式 |
| SSH Key | 私钥登录 | 已配置密钥的环境 |
| PVE API Token | `user@pve!tokenid=secret` | PVE 集群管理 |

### 安全设计

| 特性 | 实现 |
|------|------|
| 登录认证 | Cookie session，首次运行强制设置 |
| TOTP 2FA | Google Authenticator / Authy |
| 密码存储 | Argon2 哈希（不可逆） |
| 数据加密 | AES-256-GCM，密钥自动管理 |
| API 鉴权 | 中间件拦截，未登录返回 401 |

### 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | `3210` | HTTP 端口 |
| `DATABASE_URL` | `sqlite:noqjj.db?mode=rwc` | 数据库路径 |
| `STATIC_DIR` | `static` | 前端静态文件目录 |

---

## English

### What is this

**Noqjj** (No Chicken-Cutting) is a PVE chicken-cutting detection platform. It can:

- 🔍 **Auto-detect** — Scan all VMs on PVE hosts to identify chicken-cutting software
- 🚀 **Auto-deploy** — Upload detection scripts to PVE hosts automatically after adding them
- 🔐 **Secure Auth** — Password + TOTP authenticator, AES-256-GCM encryption
- 🔑 **Multiple Auth** — SSH password / SSH Key / PVE API Token
- 📱 **Real-time Alerts** — Telegram + WeChat notifications
- 🎨 **Apple Design** — Clean and elegant UI

### What is "Chicken-Cutting"

"Chicken-cutting" (切鸡) is an IDC industry term. It refers to installing virtualization software (like Incus/LXD) inside a VPS, then splitting one VPS into multiple smaller ones for resale. This is a violation for IDC operators, causing resource overselling, performance degradation, and security risks.

### Detection Methods

| Method | Condition | What it checks |
|--------|-----------|----------------|
| **GA Mode** | VM has qemu-guest-agent installed | Files, systemd services, bash_history, network |
| **Disk Mount** | GA unavailable | Copy disk → qemu-nbd read-only mount → scan filesystem |

### Detection Signatures

| Type | Path / Name |
|------|-------------|
| Incus binary | `/opt/incus/` |
| shlii.io agent | `/usr/local/bin/incushlii-agent` |
| NodeHatch agent | `/usr/local/bin/nodeget-agent` |
| systemd services | `incushlii-agent.service`, `nodeget-agent.service`, etc. |
| bash_history | `shlii.io`, `nodeget`, `incushlii` keywords |

### Quick Start

```bash
# 1. Clone
git clone https://github.com/MOSSDATA-NETWORK/Noqjj.git
cd Noqjj

# 2. Build frontend
cd frontend && npm install && npm run build && cd ..

# 3. Build backend
cd backend && cargo build --release && cd ..

# 4. Deploy
scp backend/target/release/chicken-detect-backend your-server:/opt/noqjj/
scp -r backend/static your-server:/opt/noqjj/

# 5. Run
ssh your-server
cd /opt/noqjj
chmod +x chicken-detect-backend
./chicken-detect-backend

# 6. Open browser http://your-server-ip:3210
```

### Workflow

```
First visit → /setup to create admin account (TOTP supported)
    ↓
  Dashboard → Add PVE host (choose auth method)
    ↓
  Auto-deploy → Detection script uploaded to PVE
    ↓
  Scan → View results → Configure alerts → Scheduled scans
```

### PVE Auth Methods

| Method | Description | Use Case |
|--------|-------------|----------|
| SSH Password | Most common | Default |
| SSH Key | Private key login | Environments with key configured |
| PVE API Token | `user@pve!tokenid=secret` | PVE cluster management |

### Security Design

| Feature | Implementation |
|---------|----------------|
| Authentication | Cookie session, mandatory on first run |
| TOTP 2FA | Google Authenticator / Authy |
| Password Storage | Argon2 hash (irreversible) |
| Data Encryption | AES-256-GCM, auto key management |
| API Auth | Middleware intercept, 401 if not logged in |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3210` | HTTP port |
| `DATABASE_URL` | `sqlite:noqjj.db?mode=rwc` | Database path |
| `STATIC_DIR` | `static` | Frontend static files directory |

---

## API

```
# Public
GET    /api/auth/check           # Check if initialized
POST   /api/auth/setup           # First-time setup
POST   /api/auth/login           # Login
POST   /api/auth/verify-totp     # Verify TOTP
POST   /api/auth/logout          # Logout

# Authenticated
POST   /api/auth/password        # Change password
GET    /api/hosts                # List hosts
POST   /api/hosts                # Add host (auto-deploys script)
PUT    /api/hosts/:id            # Update host
DELETE /api/hosts/:id            # Delete host
POST   /api/hosts/:id/test      # Test SSH connection
POST   /api/hosts/:id/deploy    # Manual deploy script
POST   /api/scans               # Create scan
GET    /api/scans               # Scan history
GET    /api/results             # Detection results
GET    /api/results/stats       # Statistics
GET    /api/notifications       # Notification config
POST   /api/notifications       # Add notification
PUT    /api/notifications/:id   # Update notification
POST   /api/notifications/test  # Test notification
```

---

## Tech Stack

| Component | Technology |
|-----------|------------|
| Backend | Rust + Axum + SQLite + tokio |
| Frontend | Vue 3 + Vite |
| UI Style | Apple Design |
| Encryption | AES-256-GCM |
| Auth | Argon2 + TOTP |
| Deploy | Single binary (embedded frontend) |

## Architecture

```
┌─────────────┐     HTTPS     ┌─────────────┐     SSH      ┌─────────────┐
│   Browser    │ ◄───────────► │   Platform   │ ◄───────────► │  PVE Hosts   │
│ Apple Design │  Cookie+TOTP  │  Rust + Vue  │  Auto-deploy  │  (unlimited) │
└─────────────┘              └──────┬──────┘              └─────────────┘
                                   │
                            ┌──────┴──────┐
                            │  SQLite DB   │
                            │  AES-256-GCM │
                            └─────────────┘
```

## License

MIT License

---

<p align="center">
  <b>禁止切鸡鸡 🐔🚫</b><br>
  <sub>Made with ❤️ for IDC operators</sub>
</p>
