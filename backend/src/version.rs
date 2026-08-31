/// 版本管理模块
/// - 编译时嵌入版本号
/// - GitHub API 检查新版本
/// - 下载更新
use serde::{Deserialize, Serialize};

/// 当前版本（编译时从 Cargo.toml 读取）
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "MOSSDATA-NETWORK/Noqjj";

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    published_at: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// 检查 GitHub 最新版本
pub async fn check_update() -> anyhow::Result<VersionInfo> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let client = reqwest::Client::new();

    let resp = client.get(&url)
        .header("User-Agent", format!("Noqjj/{}", VERSION))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(VersionInfo {
            current: VERSION.to_string(),
            latest: None,
            update_available: false,
            release_url: None,
            release_notes: None,
            published_at: None,
        });
    }

    let release: GithubRelease = resp.json().await?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let update_available = is_newer(&latest_version, VERSION);

    Ok(VersionInfo {
        current: VERSION.to_string(),
        latest: Some(latest_version),
        update_available,
        release_url: Some(release.html_url),
        release_notes: release.body,
        published_at: Some(release.published_at),
    })
}

/// 获取最近的更新日志（最近5个 release）
pub async fn get_changelog() -> anyhow::Result<Vec<ChangelogEntry>> {
    let url = format!("https://api.github.com/repos/{}/releases?per_page=5", GITHUB_REPO);
    let client = reqwest::Client::new();

    let resp = client.get(&url)
        .header("User-Agent", format!("Noqjj/{}", VERSION))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(vec![]);
    }

    let releases: Vec<GithubRelease> = resp.json().await?;

    Ok(releases.into_iter().map(|r| ChangelogEntry {
        version: r.tag_name.trim_start_matches('v').to_string(),
        name: r.name.unwrap_or_else(|| r.tag_name.clone()),
        notes: r.body.unwrap_or_default(),
        url: r.html_url,
        published_at: r.published_at,
    }).collect())
}

#[derive(Debug, Serialize)]
pub struct ChangelogEntry {
    pub version: String,
    pub name: String,
    pub notes: String,
    pub url: String,
    pub published_at: String,
}

/// 执行更新：下载 tarball，解压替换，自动重启
pub async fn perform_update() -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let client = reqwest::Client::new();

    let resp = client.get(&url)
        .header("User-Agent", format!("Noqjj/{}", VERSION))
        .send()
        .await?;

    let release: GithubRelease = resp.json().await?;

    // 匹配 tarball 资产名
    let asset_name = if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "noqjj-linux-x86_64.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "noqjj-linux-aarch64.tar.gz"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "noqjj-macos-aarch64.tar.gz"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "noqjj-macos-x86_64.tar.gz"
    } else {
        return Err(anyhow::anyhow!("当前平台不支持自动更新"));
    };

    let asset = release.assets.iter().find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow::anyhow!("未找到适合当前平台的更新包: {}", asset_name))?;

    let current_exe = std::env::current_exe()?;
    let install_dir = current_exe.parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取安装目录"))?;

    // 下载 tarball 到临时文件
    let tarball_path = install_dir.join("update.tar.gz");
    let resp = client.get(&asset.browser_download_url)
        .header("User-Agent", format!("Noqjj/{}", VERSION))
        .send()
        .await?;

    let bytes = resp.bytes().await?;
    tokio::fs::write(&tarball_path, &bytes).await?;

    // 解压到临时目录
    let tmp_dir = install_dir.join("_update_tmp");
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    tokio::fs::create_dir_all(&tmp_dir).await?;

    let tarball_path_clone = tarball_path.clone();
    let tmp_dir_clone = tmp_dir.clone();
    tokio::task::spawn_blocking(move || {
        let tar_gz = std::fs::File::open(&tarball_path_clone)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&tmp_dir_clone)
    }).await??;

    // 备份旧二进制
    let backup_path = current_exe.with_extension("bak");
    let _ = tokio::fs::copy(&current_exe, &backup_path).await;

    // 替换二进制（先删再写，避免 Text file busy）
    let new_binary = tmp_dir.join("noqjj");
    if new_binary.exists() {
        let _ = tokio::fs::remove_file(&current_exe).await;
        tokio::fs::copy(&new_binary, &current_exe).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&current_exe, std::fs::Permissions::from_mode(0o755)).await?;
        }
    }

    // 替换静态文件
    let new_static = tmp_dir.join("static");
    if new_static.exists() {
        let dest_static = install_dir.join("static");
        let _ = tokio::fs::remove_dir_all(&dest_static).await;
        copy_dir_recursive(&new_static, &dest_static).await?;
    }

    // 清理临时文件
    let _ = tokio::fs::remove_file(&tarball_path).await;
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;

    // 后台延迟重启（让 HTTP 响应先发出）
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        // 尝试 systemctl restart，如果是 systemd 管理的话
        let _ = tokio::process::Command::new("systemctl")
            .args(["restart", "noqjj"])
            .spawn();
    });

    Ok(format!("已更新到 {}，正在重启...", release.tag_name))
}

/// 递归复制目录
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>> {
    let src = src.to_path_buf();
    let dst = dst.to_path_buf();
    Box::pin(async move {
        tokio::fs::create_dir_all(&dst).await?;
        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if entry.file_type().await?.is_dir() {
                copy_dir_recursive(&src_path, &dst_path).await?;
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
        Ok(())
    })
}

/// 比较版本号（语义化版本）
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v.split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (
            parts.get(0).copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };

    let l = parse(latest);
    let c = parse(current);

    l.0 > c.0 || (l.0 == c.0 && l.1 > c.1) || (l.0 == c.0 && l.1 == c.1 && l.2 > c.2)
}
