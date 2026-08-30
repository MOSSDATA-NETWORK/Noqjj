<p align="center">
  <img src="https://img.shields.io/badge/Noqjj-禁止切鸡鸡-red?style=for-the-badge" alt="Noqjj">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Vue.js-35495E?style=for-the-badge&logo=vuedotjs&logoColor=4FC08D" alt="Vue.js">
  <img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/badge/Version-0.5.0-green?style=for-the-badge" alt="Version">
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
  <a href="#api">API</a> •
  <a href="#screenshots">截图</a>
</p>

---

## 中文

### 这是什么

**Noqjj**（禁止切鸡鸡）是一个 PVE 切鸡检测平台。它可以：

- 🔍 **自动检测** — 扫描 PVE 宿主机上的所有 VM，识别是否安装了切鸡软件
- 🚀 **自动部署** — 添加 PVE 主机后，自动上传检测脚本，无需手动操作
- 🔐 **多种认证** — 密码 + TOTP 身份验证器 + Passkey（指纹/面容/安全密钥）
- 🔑 **灵活接入** — SSH 密码 / SSH 私钥（拖拽上传） / PVE API Token
- 📱 **实时告警** — Telegram + 企业微信，新发现或已清除即时通知
- 🔄 **在线更新** — 自动检查新版本，一键更新
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

**方式一：下载预编译二进制 + 一键部署（推荐）**

```bash
# 下载（以 Linux x86_64 为例）
wget https://github.com/MOSSDATA-NETWORK/Noqjj/releases/download/v0.3.0/noqjj-linux-x86_64 -O noqjj
chmod +x noqjj

# 下载部署脚本
wget https://raw.githubusercontent.com/MOSSDATA-NETWORK/Noqjj/main/deploy/install.sh

# 一键部署（默认端口 3210）
bash install.sh

# 或指定端口
bash install.sh 8080
```

部署脚本会自动：
- 复制文件到 `/opt/noqjj/`
- 创建 systemd 服务
- 启用开机自启
- 启动服务

**方式二：手动部署**

```bash
# 下载二进制
wget https://github.com/MOSSDATA-NETWORK/Noqjj/releases/download/v0.3.0/noqjj-linux-x86_64 -O /opt/noqjj/noqjj
chmod +x /opt/noqjj/noqjj

# 创建 systemd 服务
cat > /etc/systemd/system/noqjj.service << 'EOF'
[Unit]
Description=Noqjj — PVE 切鸡检测平台
After=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/noqjj
ExecStart=/opt/noqjj/noqjj
Restart=on-failure
RestartSec=5
Environment=PORT=3210
Environment=STATIC_DIR=/opt/noqjj/static
Environment=DATABASE_URL=sqlite:/opt/noqjj/noqjj.db?mode=rwc

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now noqjj
```

**方式三：从源码编译**

```bash
git clone https://github.com/MOSSDATA-NETWORK/Noqjj.git
cd Noqjj/frontend && npm install && npm run build && cd ../backend
cargo build --release
# 产物：backend/target/release/chicken-detect-backend
```

**常用命令**

```bash
systemctl status noqjj    # 查看状态
systemctl restart noqjj   # 重启
systemctl stop noqjj      # 停止
journalctl -u noqjj -f    # 查看日志
```

### HTTPS 配置

**本系统处理密码、SSH 私钥等敏感数据，强烈建议强制 HTTPS。**

**方案一：Nginx 反代 + Let's Encrypt（推荐）**

```bash
# 1. 安装 nginx 和 certbot
apt install nginx certbot python3-certbot-nginx

# 2. 申请证书
certbot --nginx -d noqjj.example.com

# 3. 复制 nginx 配置
cp deploy/nginx.conf /etc/nginx/sites-available/noqjj
# 编辑修改 server_name 为你的域名
vi /etc/nginx/sites-available/noqjj
ln -s /etc/nginx/sites-available/noqjj /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx

# 4. 修改 Noqjj 只监听 localhost（可选，更安全）
# 编辑 systemd 服务，添加 Environment=HOST=127.0.0.1
systemctl edit noqjj
# 添加:
# [Service]
# Environment=HOST=127.0.0.1
systemctl restart noqjj
```

**方案二：内置 TLS（无需 nginx）**

```bash
# 1. 准备证书（Let's Encrypt 或自签名）
certbot certonly --standalone -d noqjj.example.com

# 2. 配置环境变量
systemctl edit noqjj
# 添加:
# [Service]
# Environment=TLS_CERT=/etc/letsencrypt/live/noqjj.example.com/fullchain.pem
# Environment=TLS_KEY=/etc/letsencrypt/live/noqjj.example.com/privkey.pem

# 3. 重启
systemctl restart noqjj
```

**方案三：自签名证书（测试用）**

```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=noqjj"

# 启动
TLS_CERT=cert.pem TLS_KEY=key.pem ./noqjj
```

**环境变量汇总**

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | `3210` | HTTP/HTTPS 端口 |
| `HOST` | `0.0.0.0` | 监听地址（nginx 反代时设 `127.0.0.1`） |
| `DATABASE_URL` | `sqlite:noqjj.db?mode=rwc` | 数据库路径 |
| `STATIC_DIR` | `static` | 前端静态文件目录 |
| `TLS_CERT` | (空) | TLS 证书路径，设置后启用 HTTPS |
| `TLS_KEY` | (空) | TLS 私钥路径，设置后启用 HTTPS |

### 使用流程

```
首次访问 → /setup 设置管理员（用户名 + 密码 + TOTP 可选）
    ↓
  登录 → 密码登录 或 Passkey 直接登录
    ↓
  控制台 → 添加 PVE 主机
    ├─ 输入名称、IP
    ├─ 选择接入方式（密码 / 私钥拖拽上传 / API Token）
    └─ 自动部署检测脚本
    ↓
  扫描 → 查看结果 → 配置通知 → 定时巡检
```

### 登录方式

| 方式 | 说明 |
|------|------|
| **密码登录** | 用户名 + 密码，可选 TOTP 二次验证 |
| **Passkey 登录** | 指纹、面容、安全密钥，无需输入密码 |

Passkey 支持的来源：
- 🍎 iCloud 钥匙串
- 🔵 Google Password Manager
- 🔒 Bitwarden
- 🔐 1Password
- 🔑 YubiKey 等硬件安全密钥
- 📱 设备指纹 / 面容

### PVE 接入方式

| 方式 | 说明 | 适用场景 |
|------|------|---------|
| SSH 密码 | 最常用 | 默认方式 |
| SSH 私钥 | 拖拽文件或粘贴内容，加密存储 | 已配置密钥的环境 |
| PVE API Token | `user@pve!tokenid=secret` | PVE 集群管理 |

### 安全设计

| 特性 | 实现 |
|------|------|
| 登录认证 | Cookie session + Passkey (WebAuthn) |
| TOTP 2FA | Google Authenticator / Authy 等 |
| Passkey | WebAuthn FIDO2，支持 iCloud/Google/Bitwarden |
| 密码存储 | Argon2 哈希（不可逆） |
| 凭据加密 | AES-256-GCM，密钥自动管理 |
| SSH 私钥 | 加密存储，临时文件用完即删 |
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
- 🔐 **Multi-auth** — Password + TOTP authenticator + Passkey (fingerprint/face/security key)
- 🔑 **Flexible Access** — SSH password / SSH key (drag & drop upload) / PVE API Token
- 📱 **Real-time Alerts** — Telegram + WeChat notifications
- 🔄 **Online Update** — Auto-check new versions, one-click update
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

**Option 1: Download pre-built binary (Recommended)**

```bash
# Download (Linux x86_64 example)
wget https://github.com/MOSSDATA-NETWORK/Noqjj/releases/download/v0.3.0/noqjj-linux-x86_64
chmod +x noqjj-linux-x86_64

# Run
./noqjj-linux-x86_64

# Open browser http://your-server-ip:3210
```

**Option 2: Build from source**

```bash
git clone https://github.com/MOSSDATA-NETWORK/Noqjj.git
cd Noqjj/frontend && npm install && npm run build && cd ../backend
cargo build --release
# Output: backend/target/release/chicken-detect-backend
```

### Workflow

```
First visit → /setup to create admin (username + password + TOTP optional)
    ↓
  Login → Password login or Passkey direct login
    ↓
  Dashboard → Add PVE host
    ├─ Enter name, IP
    ├─ Choose auth method (password / key drag & drop / API Token)
    └─ Auto-deploy detection script
    ↓
  Scan → View results → Configure alerts → Scheduled scans
```

### Login Methods

| Method | Description |
|--------|-------------|
| **Password** | Username + password, optional TOTP 2FA |
| **Passkey** | Fingerprint, face, security key — no password needed |

Passkey supported sources:
- 🍎 iCloud Keychain
- 🔵 Google Password Manager
- 🔒 Bitwarden
- 🔐 1Password
- 🔑 YubiKey and other hardware security keys
- 📱 Device fingerprint / face

### PVE Auth Methods

| Method | Description | Use Case |
|--------|-------------|----------|
| SSH Password | Most common | Default |
| SSH Key | Drag & drop file or paste content, encrypted storage | Environments with key configured |
| PVE API Token | `user@pve!tokenid=secret` | PVE cluster management |

### Security Design

| Feature | Implementation |
|---------|----------------|
| Authentication | Cookie session + Passkey (WebAuthn) |
| TOTP 2FA | Google Authenticator / Authy etc. |
| Passkey | WebAuthn FIDO2, supports iCloud/Google/Bitwarden |
| Password Storage | Argon2 hash (irreversible) |
| Credential Encryption | AES-256-GCM, auto key management |
| SSH Key | Encrypted storage, temp file deleted after use |
| API Auth | Middleware intercept, 401 if not logged in |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3210` | HTTP/HTTPS port |
| `HOST` | `0.0.0.0` | Listen address (set `127.0.0.1` for nginx reverse proxy) |
| `DATABASE_URL` | `sqlite:noqjj.db?mode=rwc` | Database path |
| `STATIC_DIR` | `static` | Frontend static files directory |
| `TLS_CERT` | (empty) | TLS cert path, enables HTTPS when set |
| `TLS_KEY` | (empty) | TLS key path, enables HTTPS when set |

### HTTPS Setup

**This system handles passwords, SSH keys, and other sensitive data. HTTPS is strongly recommended.**

**Option 1: Nginx reverse proxy + Let's Encrypt (Recommended)**

```bash
apt install nginx certbot python3-certbot-nginx
certbot --nginx -d noqjj.example.com
cp deploy/nginx.conf /etc/nginx/sites-available/noqjj
# Edit server_name to your domain
ln -s /etc/nginx/sites-available/noqjj /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx
```

**Option 2: Built-in TLS (No nginx needed)**

```bash
systemctl edit noqjj
# Add:
# [Service]
# Environment=TLS_CERT=/etc/letsencrypt/live/noqjj.example.com/fullchain.pem
# Environment=TLS_KEY=/etc/letsencrypt/live/noqjj.example.com/privkey.pem
systemctl restart noqjj
```

---

## API

```
# Public
GET    /api/auth/check           # Check if initialized
POST   /api/auth/setup           # First-time setup
POST   /api/auth/login           # Login (password)
POST   /api/auth/verify-totp     # Verify TOTP
POST   /api/auth/logout          # Logout
POST   /api/passkey/has          # Check if user has Passkey
POST   /api/passkey/login/start  # Passkey login: get challenge
POST   /api/passkey/login/finish # Passkey login: verify

# Authenticated
POST   /api/auth/password        # Change password
POST   /api/auth/reset-totp      # Reset TOTP (re-bind)
POST   /api/auth/disable-totp    # Disable TOTP
POST   /api/passkey/register/start  # Passkey register: get challenge
POST   /api/passkey/register/finish # Passkey register: verify & store
POST   /api/passkey/delete          # Delete Passkey
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
GET    /api/version             # Current version
GET    /api/version/check       # Check for updates
GET    /api/version/changelog   # Changelog
POST   /api/version/update      # Perform update
```

---

## Tech Stack

| Component | Technology |
|-----------|------------|
| Backend | Rust + Axum + SQLite + tokio |
| Frontend | Vue 3 + Vite |
| UI Style | Apple Design |
| Encryption | AES-256-GCM |
| Auth | Argon2 + TOTP + Passkey (WebAuthn FIDO2) |
| Deploy | Single binary (embedded frontend) |

## Architecture

```
┌─────────────┐     HTTPS     ┌─────────────┐     SSH      ┌─────────────┐
│   Browser    │ ◄───────────► │   Platform   │ ◄───────────► │  PVE Hosts   │
│ Apple Design │ Cookie+Passkey│  Rust + Vue  │  Auto-deploy  │  (unlimited) │
└─────────────┘    +TOTP       └──────┬──────┘              └─────────────┘
                                   │
                            ┌──────┴──────┐
                            │  SQLite DB   │
                            │  AES-256-GCM │
                            └─────────────┘
```

## Releases

| Version | Date | Highlights |
|---------|------|------------|
| v0.5.0 | 2026-08-31 | 安全加固：CORS 白名单、Master Key 环境变量、登录频率限制、Session IP 绑定 |
| v0.4.0 | 2026-08-31 | HTTPS 支持、Nginx 反代教程、部署脚本 |
| v0.3.0 | 2026-08-31 | Passkey 登录、SSH 私钥拖拽上传、账户设置 |
| v0.2.0 | 2026-08-30 | TOTP、自动部署、多接入方式、Apple Design |

## License

MIT License

---

<p align="center">
  <b>禁止切鸡鸡 🐔🚫</b><br>
  <sub>Made with ❤️ for IDC operators</sub>
</p>
