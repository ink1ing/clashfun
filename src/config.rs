use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub subscription_url: Option<String>,
    pub selected_node: Option<String>,
    pub proxy_port: u16,
    pub auto_select: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            subscription_url: None,
            selected_node: None,
            proxy_port: 7890,
            auto_select: true,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|dir| dir.join("clashfun"))
            .context("无法获取配置目录")
    }

    pub fn config_file() -> Result<PathBuf> {
        Self::config_dir().map(|dir| dir.join("config.yaml"))
    }

    pub fn load() -> Result<Self> {
        let config_file = Self::config_file()?;

        if !config_file.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_file)
            .with_context(|| format!("无法读取配置文件: {:?}", config_file))?;

        let config: Self = serde_yaml::from_str(&content)
            .with_context(|| format!("无法解析配置文件: {:?}", config_file))?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir()?;
        let config_file = Self::config_file()?;

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("无法创建配置目录: {:?}", config_dir))?;
        }

        let content = serde_yaml::to_string(self)
            .context("无法序列化配置")?;

        fs::write(&config_file, content)
            .with_context(|| format!("无法写入配置文件: {:?}", config_file))?;

        Ok(())
    }
}