use anyhow::{Result, anyhow};
use log::{info, warn, error};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::env;

const GITHUB_API_URL: &str = "https://api.github.com/repos/ink1ing/clashfun/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
    prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
}

pub struct Updater {
    client: reqwest::Client,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 检查是否有可用更新
    pub async fn check_for_updates(&self) -> Result<UpdateInfo> {
        info!("正在检查更新...");

        let response = self.client
            .get(GITHUB_API_URL)
            .header("User-Agent", format!("ClashFun/{}", CURRENT_VERSION))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("获取版本信息失败: HTTP {}", response.status()));
        }

        let release: GitHubRelease = response.json().await?;

        // 跳过预发布版本
        if release.prerelease {
            return Ok(UpdateInfo {
                current_version: CURRENT_VERSION.to_string(),
                latest_version: None,
                update_available: false,
                download_url: None,
                release_notes: None,
            });
        }

        let latest_version = release.tag_name.trim_start_matches('v');
        let update_available = self.version_compare(CURRENT_VERSION, latest_version)?;

        let download_url = if update_available {
            self.get_download_url(&release.assets)?
        } else {
            None
        };

        Ok(UpdateInfo {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: Some(latest_version.to_string()),
            update_available,
            download_url,
            release_notes: release.body,
        })
    }

    /// 比较版本号，返回是否需要更新
    fn version_compare(&self, current: &str, latest: &str) -> Result<bool> {
        let current_parts: Vec<u32> = current.split('.')
            .map(|s| s.parse().unwrap_or(0))
            .collect();
        let latest_parts: Vec<u32> = latest.split('.')
            .map(|s| s.parse().unwrap_or(0))
            .collect();

        let max_len = current_parts.len().max(latest_parts.len());

        for i in 0..max_len {
            let curr = current_parts.get(i).unwrap_or(&0);
            let latest = latest_parts.get(i).unwrap_or(&0);

            if latest > curr {
                return Ok(true);
            } else if latest < curr {
                return Ok(false);
            }
        }

        Ok(false)
    }

    /// 获取适合当前平台的下载URL
    fn get_download_url(&self, assets: &[GitHubAsset]) -> Result<Option<String>> {
        let os = env::consts::OS;
        let arch = env::consts::ARCH;

        // 构建匹配的文件名模式
        let patterns = match (os, arch) {
            ("macos", "aarch64") => vec!["darwin-aarch64", "macos-arm64", "apple-silicon"],
            ("macos", "x86_64") => vec!["darwin-x86_64", "macos-x64", "intel-mac"],
            ("linux", "x86_64") => vec!["linux-x86_64", "linux-amd64"],
            ("linux", "aarch64") => vec!["linux-aarch64", "linux-arm64"],
            ("windows", "x86_64") => vec!["windows-x86_64", "win64"],
            _ => return Err(anyhow!("不支持的平台: {}-{}", os, arch)),
        };

        // 查找匹配的资源
        for asset in assets {
            for pattern in &patterns {
                if asset.name.to_lowercase().contains(pattern) {
                    return Ok(Some(asset.browser_download_url.clone()));
                }
            }
        }

        Err(anyhow!("未找到适合当前平台的下载文件"))
    }

    /// 执行更新
    pub async fn perform_update(&self, download_url: &str) -> Result<()> {
        println!("🔄 正在下载最新版本...");

        // 获取当前可执行文件路径
        let current_exe = env::current_exe()?;
        let temp_dir = env::temp_dir();
        let temp_file = temp_dir.join("cf_new");

        // 下载新版本
        let response = self.client
            .get(download_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("下载失败: HTTP {}", response.status()));
        }

        let bytes = response.bytes().await?;

        // 检查是否是压缩文件
        if download_url.ends_with(".tar.gz") || download_url.ends_with(".zip") {
            self.extract_archive(&bytes, &temp_file).await?;
        } else {
            fs::write(&temp_file, bytes)?;
        }

        // 设置执行权限 (Unix系统)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&temp_file)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_file, perms)?;
        }

        println!("✅ 下载完成，正在替换旧版本...");

        // 清理可能存在的旧版本
        self.cleanup_old_versions(&current_exe).await?;

        // 备份当前版本
        let backup_path = format!("{}.backup", current_exe.display());
        if let Err(e) = fs::copy(&current_exe, &backup_path) {
            warn!("备份当前版本失败: {}", e);
        }

        // 替换可执行文件
        self.replace_executable(&temp_file, &current_exe).await?;

        // 删除临时文件
        let _ = fs::remove_file(&temp_file);

        println!("🎉 更新完成！");
        println!("💡 请重新运行 cf 命令以使用新版本");

        Ok(())
    }

    /// 提取压缩文件
    async fn extract_archive(&self, bytes: &[u8], output_path: &Path) -> Result<()> {
        // 这里简化处理，假设压缩包中直接包含cf可执行文件
        // 实际实现可能需要使用tar或zip库
        return Err(anyhow!("暂不支持压缩包格式，请直接下载可执行文件"));
    }

    /// 清理旧版本和重复安装
    async fn cleanup_old_versions(&self, current_exe: &Path) -> Result<()> {
        let exe_dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
        let exe_name = current_exe.file_name().unwrap_or_else(|| std::ffi::OsStr::new("cf"));

        // 查找可能的重复安装
        let patterns = vec![
            "cf",
            "clashfun",
            "cf.exe",
            "clashfun.exe",
            "cf.backup",
            "cf.old",
        ];

        for entry in fs::read_dir(exe_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // 跳过当前执行文件
            if file_name == exe_name {
                continue;
            }

            // 检查是否是旧版本文件
            for pattern in &patterns {
                if file_name_str.contains(pattern) && file_name_str.contains("backup") || file_name_str.contains("old") {
                    if let Err(e) = fs::remove_file(entry.path()) {
                        warn!("删除旧版本文件失败 {}: {}", entry.path().display(), e);
                    } else {
                        info!("已删除旧版本文件: {}", entry.path().display());
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// 替换可执行文件
    async fn replace_executable(&self, new_exe: &Path, current_exe: &Path) -> Result<()> {
        // 在Windows上可能需要特殊处理
        #[cfg(windows)]
        {
            // Windows上可能需要使用批处理脚本来延迟替换
            let batch_script = format!(
                r#"
@echo off
timeout /t 1 /nobreak >nul
move "{}" "{}"
del "%~f0"
"#,
                new_exe.display(),
                current_exe.display()
            );

            let batch_path = env::temp_dir().join("cf_update.bat");
            fs::write(&batch_path, batch_script)?;

            Command::new("cmd")
                .args(["/c", "start", "", batch_path.to_str().unwrap()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }

        #[cfg(not(windows))]
        {
            // Unix系统直接替换
            fs::copy(new_exe, current_exe)?;
        }

        Ok(())
    }

    /// 检查是否有多个版本冲突
    pub async fn check_version_conflicts(&self) -> Result<Vec<PathBuf>> {
        let mut conflicts = Vec::new();

        // 检查常见的安装路径
        let common_paths = vec![
            "/usr/local/bin/cf",
            "/usr/bin/cf",
            "/opt/clashfun/cf",
            &format!("{}/.local/bin/cf", env::var("HOME").unwrap_or_default()),
        ];

        for path_str in common_paths {
            let path = Path::new(path_str);
            if path.exists() && path != env::current_exe()? {
                conflicts.push(path.to_path_buf());
            }
        }

        // 检查PATH中的其他cf命令
        if let Ok(which_output) = Command::new("which")
            .arg("-a")
            .arg("cf")
            .output() {

            let output_str = String::from_utf8_lossy(&which_output.stdout);
            for line in output_str.lines() {
                let path = Path::new(line.trim());
                if path.exists() && path != env::current_exe()? {
                    if !conflicts.contains(&path.to_path_buf()) {
                        conflicts.push(path.to_path_buf());
                    }
                }
            }
        }

        Ok(conflicts)
    }
}