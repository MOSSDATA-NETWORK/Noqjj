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

/// SSH 命令执行
pub async fn ssh_exec(host: &str, port: u16, user: &str, auth: &SshAuth, cmd: &str) -> anyhow::Result<String> {
    let mut key_file_path: Option<String> = None;
    let mut base_args = vec![
        "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(), "ConnectTimeout=10".to_string(),
        "-p".to_string(), port.to_string(),
    ];

    match auth {
        SshAuth::Password(pass) => {
            let ssh_cmd = format!(
                "ssh {} {}@{} '{}'",
                base_args.join(" "),
                user, host,
                cmd.replace('\'', "'\\''")
            );
            let output = Command::new("sshpass")
                .args(["-p", pass, "bash", "-c", &ssh_cmd])
                .output()
                .await
                .map_err(|_| anyhow::anyhow!("sshpass 未安装"))?;
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
        SshAuth::KeyContent(key) => {
            let path = write_temp_key(key).await?;
            key_file_path = Some(path.clone());
            base_args.push("-i".to_string());
            base_args.push(path);
            base_args.push("-o".to_string());
            base_args.push("BatchMode=yes".to_string());
        }
        SshAuth::None => {
            base_args.push("-o".to_string());
            base_args.push("BatchMode=yes".to_string());
        }
    }

    base_args.push(format!("{}@{}", user, host));
    base_args.push(cmd.to_string());

    let output = Command::new("ssh")
        .args(&base_args)
        .output()
        .await?;

    // 清理临时密钥文件
    if let Some(path) = key_file_path {
        let _ = tokio::fs::remove_file(path).await;
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// SCP 上传文件
async fn scp_upload(host: &str, port: u16, user: &str, auth: &SshAuth, local_path: &str, remote_path: &str) -> anyhow::Result<()> {
    let mut key_file_path: Option<String> = None;
    let mut base_args = vec![
        "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
        "-P".to_string(), port.to_string(),
    ];

    match auth {
        SshAuth::Password(pass) => {
            let output = Command::new("sshpass")
                .args(["-p", pass, "scp"])
                .args(&base_args)
                .args([local_path, &format!("{}@{}:{}", user, host, remote_path)])
                .output()
                .await
                .map_err(|_| anyhow::anyhow!("sshpass 未安装"))?;
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
        Some(id) => format!("chicken-check --vmid {}", id),
        None => "chicken-check --all".to_string(),
    };
    let result = ssh_exec(host, port, user, auth, &cmd).await?;
    Ok(result)
}
