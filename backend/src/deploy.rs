/// 自动部署检测脚本到 PVE 宿主机
use tokio::process::Command;

const SCRIPT_CONTENT: &str = include_str!("../scripts/chicken-check.sh");

/// SSH 命令执行
async fn ssh_exec(host: &str, port: u16, user: &str, password: Option<&str>, cmd: &str) -> anyhow::Result<String> {
    if let Some(pass) = password {
        let ssh_cmd = format!(
            "ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 -p {} {}@{} '{}'",
            port, user, host, cmd.replace('\'', "'\\''")
        );
        let output = Command::new("sshpass")
            .args(["-p", pass, "bash", "-c", &ssh_cmd])
            .output()
            .await
            .map_err(|_| anyhow::anyhow!("sshpass 未安装"))?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let output = Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=10",
                "-o", "BatchMode=yes",
                "-p", &port.to_string(),
                &format!("{}@{}", user, host),
                cmd,
            ])
            .output()
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// SCP 上传文件
async fn scp_upload(host: &str, port: u16, user: &str, password: Option<&str>, local_path: &str, remote_path: &str) -> anyhow::Result<()> {
    if let Some(pass) = password {
        let output = Command::new("sshpass")
            .args(["-p", pass, "scp", "-o", "StrictHostKeyChecking=no", "-P", &port.to_string(), local_path, &format!("{}@{}:{}", user, host, remote_path)])
            .output()
            .await
            .map_err(|_| anyhow::anyhow!("sshpass 未安装"))?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("SCP 失败: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else {
        let output = Command::new("scp")
            .args(["-o", "StrictHostKeyChecking=no", "-P", &port.to_string(), local_path, &format!("{}@{}:{}", user, host, remote_path)])
            .output()
            .await?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("SCP 失败: {}", String::from_utf8_lossy(&output.stderr)));
        }
    }
    Ok(())
}

/// 部署检测脚本到 PVE 主机
pub async fn deploy_script(host: &str, port: u16, user: &str, password: Option<&str>) -> anyhow::Result<()> {
    // 写临时文件
    let tmp_path = format!("/tmp/chicken-check-{}.sh", uuid::Uuid::new_v4());
    tokio::fs::write(&tmp_path, SCRIPT_CONTENT).await?;

    // 上传
    let remote_path = "/usr/local/bin/chicken-check";
    scp_upload(host, port, user, password, &tmp_path, remote_path).await?;

    // 设置执行权限
    ssh_exec(host, port, user, password, &format!("chmod +x {}", remote_path)).await?;

    // 验证安装
    let result = ssh_exec(host, port, user, password, "chicken-check --check-agent").await?;
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
pub async fn run_remote_scan(host: &str, port: u16, user: &str, password: Option<&str>, vmid: Option<&str>) -> anyhow::Result<String> {
    let cmd = match vmid {
        Some(id) => format!("chicken-check --vmid {}", id),
        None => "chicken-check --all".to_string(),
    };
    let result = ssh_exec(host, port, user, password, &cmd).await?;
    Ok(result)
}
