/// 自动部署检测脚本到 PVE 宿主机
/// 支持密码和 SSH 私钥认证
use tokio::process::Command;

const SCRIPT_CONTENT: &str = include_str!("../scripts/chicken-check.sh");

/// SSH 认证方式
pub enum SshAuth {
    Password(String),
    KeyContent(String),
    None,
}

impl SshAuth {
    /// 从 Host 信息构建认证方式
    pub fn from_host(password_enc: Option<&str>, key_enc: Option<&str>, master_key: &[u8]) -> Self {
        // 优先用私钥
        if let Some(enc) = key_enc {
            if let Ok(key) = crate::crypto::decrypt(enc, master_key) {
                return SshAuth::KeyContent(key);
            }
        }
        if let Some(enc) = password_enc {
            if let Ok(pass) = crate::crypto::decrypt(enc, master_key) {
                return SshAuth::Password(pass);
            }
        }
        SshAuth::None
    }
}

/// 写私钥到临时文件，返回路径
async fn write_temp_key(key_content: &str) -> anyhow::Result<String> {
    let tmp_path = format!("/tmp/ssh-key-{}", uuid::Uuid::new_v4());
    tokio::fs::write(&tmp_path, key_content).await?;

    // 设置权限为 600（SSH 要求）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600)).await?;
    }

    Ok(tmp_path)
}

/// 验证主机名/用户名格式（防止命令注入）
fn validate_ssh_target(s: &str) -> anyhow::Result<()> {
    if s.is_empty() || s.len() > 253 {
        return Err(anyhow::anyhow!("主机名长度无效"));
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '@') {
        return Err(anyhow::anyhow!("主机名包含非法字符"));
    }
    Ok(())
}

/// SSH 命令执行
pub async fn ssh_exec(host: &str, port: u16, user: &str, auth: &SshAuth, cmd: &str) -> anyhow::Result<String> {
    validate_ssh_target(host)?;
    validate_ssh_target(user)?;

    let mut key_file_path: Option<String> = None;
    let mut args = vec![
        "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(), "ConnectTimeout=10".to_string(),
        "-o".to_string(), "ServerAliveInterval=15".to_string(),
        "-o".to_string(), "ServerAliveCountMax=4".to_string(),
        "-p".to_string(), port.to_string(),
    ];

    match auth {
        SshAuth::Password(pass) => {
            // 密码模式：不用 BatchMode（会禁用密码认证）
            // 写密码到临时文件，避免进程列表泄露
            let pass_file = format!("/tmp/ssh-pass-{}", uuid::Uuid::new_v4());
            tokio::fs::write(&pass_file, pass).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(&pass_file, std::fs::Permissions::from_mode(0o600)).await?;
            }

            args.push("-i".to_string());
            args.push("/dev/null".to_string()); // 不用密钥文件
            args.push(format!("{}@{}", user, host));
            args.push(cmd.to_string());

            let output = Command::new("sshpass")
                .args(["-f", &pass_file])
                .arg("ssh")
                .args(&args)
                .output()
                .await;

            let _ = tokio::fs::remove_file(&pass_file).await;
            let output = output.map_err(|_| anyhow::anyhow!("sshpass 未安装"))?;
            if !output.status.success() && output.stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("SSH 执行失败: {}", stderr.trim()));
            }
            // 只返回 stdout，忽略 stderr
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
        SshAuth::KeyContent(key) => {
            // 密钥模式：可以用 BatchMode
            args.push("-o".to_string());
            args.push("BatchMode=yes".to_string());
            let path = write_temp_key(key).await?;
            key_file_path = Some(path.clone());
            args.push("-i".to_string());
            args.push(path);
        }
        SshAuth::None => {
            args.push("-o".to_string());
            args.push("BatchMode=yes".to_string());
        }
    }

    args.push(format!("{}@{}", user, host));
    args.push(cmd.to_string());

    let output = Command::new("ssh")
        .args(&args)
        .output()
        .await?;

    if let Some(path) = key_file_path {
        let _ = tokio::fs::remove_file(path).await;
    }

    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("SSH 执行失败: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// SCP 上传文件
async fn scp_upload(host: &str, port: u16, user: &str, auth: &SshAuth, local_path: &str, remote_path: &str) -> anyhow::Result<()> {
    validate_ssh_target(host)?;
    validate_ssh_target(user)?;

    let mut key_file_path: Option<String> = None;
    let mut pass_file_path: Option<String> = None;
    let mut base_args = vec![
        "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
        "-P".to_string(), port.to_string(),
    ];

    match auth {
        SshAuth::Password(pass) => {
            let pass_file = format!("/tmp/ssh-pass-{}", uuid::Uuid::new_v4());
            tokio::fs::write(&pass_file, pass).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(&pass_file, std::fs::Permissions::from_mode(0o600)).await?;
            }
            pass_file_path = Some(pass_file.clone());

            let output = Command::new("sshpass")
                .args(["-f", &pass_file, "scp"])
                .args(&base_args)
                .args([local_path, &format!("{}@{}:{}", user, host, remote_path)])
                .output()
                .await
                .map_err(|_| anyhow::anyhow!("sshpass 未安装"))?;

            if let Some(p) = pass_file_path { let _ = tokio::fs::remove_file(p).await; }
            if !output.status.success() {
                return Err(anyhow::anyhow!("SCP 失败: {}", String::from_utf8_lossy(&output.stderr)));
            }
            return Ok(());
        }
        SshAuth::KeyContent(key) => {
            let path = write_temp_key(key).await?;
            key_file_path = Some(path.clone());
            base_args.push("-i".to_string());
            base_args.push(path);
        }
        SshAuth::None => {}
    }

    let output = Command::new("scp")
        .args(&base_args)
        .args([local_path, &format!("{}@{}:{}", user, host, remote_path)])
        .output()
        .await?;

    if let Some(path) = key_file_path {
        let _ = tokio::fs::remove_file(path).await;
    }

    if !output.status.success() {
        return Err(anyhow::anyhow!("SCP 失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

/// 测试 SSH 连接
pub async fn test_ssh(host: &str, port: u16, user: &str, auth: &SshAuth) -> anyhow::Result<()> {
    let result = ssh_exec(host, port, user, auth, "echo ok").await?;
    if result.trim() == "ok" {
        Ok(())
    } else {
        Err(anyhow::anyhow!("SSH 连接失败: {}", result.trim()))
    }
}

/// 部署检测脚本到 PVE 主机
pub async fn deploy_script(host: &str, port: u16, user: &str, auth: &SshAuth) -> anyhow::Result<()> {
    // 写临时脚本文件
    let tmp_path = format!("/tmp/chicken-check-{}.sh", uuid::Uuid::new_v4());
    tokio::fs::write(&tmp_path, SCRIPT_CONTENT).await?;

    // 上传
    let remote_path = "/usr/local/bin/chicken-check";
    scp_upload(host, port, user, auth, &tmp_path, remote_path).await?;

    // 设置执行权限
    ssh_exec(host, port, user, auth, &format!("chmod +x {}", remote_path)).await?;

    // 验证安装
    let result = ssh_exec(host, port, user, auth, "chicken-check --check-agent").await?;
    if result.contains("\"agent\":\"installed\"") {
        tracing::info!("检测脚本已部署到 {}@{}", user, host);
    } else {
        return Err(anyhow::anyhow!("脚本部署验证失败"));
    }

    // 清理临时文件
    let _ = tokio::fs::remove_file(&tmp_path).await;

    Ok(())
}

/// 远程执行检测
pub async fn run_remote_scan(host: &str, port: u16, user: &str, auth: &SshAuth, vmid: Option<&str>) -> anyhow::Result<String> {
    let cmd = match vmid {
        // 单台扫描由「磁盘扫描」按钮触发，强制磁盘挂载模式（GA被禁/无GA都能查）
        Some(id) => format!("chicken-check --vmid {} --disk", id),
        None => "chicken-check --all".to_string(),
    };
    let result = ssh_exec(host, port, user, auth, &cmd).await?;
    Ok(result)
}

/// 远程执行任意命令
pub async fn run_remote_cmd(host: &str, port: u16, user: &str, auth: &SshAuth, cmd: &str) -> anyhow::Result<String> {
    let result = ssh_exec(host, port, user, auth, cmd).await?;
    Ok(result)
}

/// 检查远程脚本版本
pub async fn check_script_version(host: &str, port: u16, user: &str, auth: &SshAuth) -> Option<String> {
    let result = ssh_exec(host, port, user, auth, "chicken-check --version 2>/dev/null || echo none").await.ok()?;
    let v = result.trim().to_string();
    if v == "none" || v.is_empty() { None } else { Some(v) }
}

/// 批量重新部署脚本到所有已部署的主机
pub async fn redeploy_all(pool: &sqlx::SqlitePool, master_key: &[u8]) -> anyhow::Result<()> {
    let hosts = crate::db::list_hosts(pool).await?;
    let current_version = crate::version::SCRIPT_VERSION;
    let mut deployed = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for host in &hosts {
        if !host.agent_deployed {
            skipped += 1;
            continue;
        }

        let auth = SshAuth::from_host(
            host.password_encrypted.as_deref(),
            host.ssh_key_encrypted.as_deref(),
            master_key,
        );

        // 检查远程脚本版本
        let remote_version = check_script_version(&host.host, host.port as u16, &host.username, &auth).await;

        if remote_version.as_deref() == Some(current_version) {
            skipped += 1;
            continue;
        }

        // 版本不同或无版本，重新部署
        tracing::info!("Redeploying script to {} (remote={}, target={})", host.name, remote_version.unwrap_or_default(), current_version);
        match deploy_script(&host.host, host.port as u16, &host.username, &auth).await {
            Ok(_) => {
                deployed += 1;
                let _ = crate::db::update_host_agent_status(pool, host.id, true).await;
            }
            Err(e) => {
                tracing::warn!("Redeploy to {} failed: {}", host.name, e);
                failed += 1;
            }
        }
    }

    tracing::info!("Script redeploy: {} deployed, {} skipped, {} failed", deployed, skipped, failed);
    Ok(())
}
