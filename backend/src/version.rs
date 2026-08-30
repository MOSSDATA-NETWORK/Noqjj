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

/// 执行更新：下载新版本二进制并替换当前文件
pub async fn perform_update() -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let client = reqwest::Client::new();

    let resp = client.get(&url)
        .header("User-Agent", format!("Noqjj/{}", VERSION))
        .send()
        .await?;

    let release: GithubRelease = resp.json().await?;

    // 找到对应平台的二进制
    let current_exe = std::env::current_exe()?;
    let asset_name = if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "noqjj-linux-x86_64"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "noqjj-linux-aarch64"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "noqjj-macos-aarch64"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "noqjj-macos-x86_64"
    } else {
        return Err(anyhow::anyhow!("当前平台不支持自动更新"));
    };

    let asset = release.assets.iter().find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow::anyhow!("未找到适合当前平台的更新包"))?;

    // 下载新版本到临时文件
    let tmp_path = current_exe.with_extension("tmp");
    let resp = client.get(&asset.browser_download_url)
        .header("User-Agent", format!("Noqjj/{}", VERSION))
        .send()
        .await?;

    let bytes = resp.bytes().await?;
    tokio::fs::write(&tmp_path, &bytes).await?;

    // 设置执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755)).await?;
    }

    // 备份旧版本
    let backup_path = current_exe.with_extension("bak");
    tokio::fs::copy(&current_exe, &backup_path).await?;

    // 替换
    tokio::fs::rename(&tmp_path, &current_exe).await?;

    Ok(format!("已更新到 {}，请重启服务生效", release.tag_name))
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
