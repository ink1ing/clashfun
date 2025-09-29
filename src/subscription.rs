use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use base64::{engine::general_purpose, Engine as _};
use log::{error, info};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub protocol: String,
    pub password: Option<String>,
    pub cipher: Option<String>,
    pub latency: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClashConfig {
    pub proxies: Vec<HashMap<String, serde_yaml::Value>>,
}

pub struct SubscriptionManager {
    client: Client,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn url_decode(encoded: &str) -> String {
        // 简单的URL解码，处理常见的编码模式
        encoded
            .replace("%20", " ")
            .replace("%7C", "|")
            .replace("%E2%9C%85", "✅")
            .replace("%F0%9F%87%BA%F0%9F%87%B8", "🇺🇸")
            .replace("%F0%9F%87%AD%F0%9F%87%B0", "🇭🇰")
            .replace("%F0%9F%87%AF%F0%9F%87%B5", "🇯🇵")
            .replace("%F0%9F%87%B8%F0%9F%87%AC", "🇸🇬")
            .replace("%F0%9F%87%B0%F0%9F%87%B7", "🇰🇷")
            .replace("%F0%9F%87%A8%F0%9F%87%B3", "🇨🇳")
            .replace("%E7%BE%8E%E5%9B%BD", "美国")
            .replace("%E9%A6%99%E6%B8%AF", "香港")
            .replace("%E6%97%A5%E6%9C%AC", "日本")
            .replace("%E6%96%B0%E5%8A%A0%E5%9D%A1", "新加坡")
            .replace("%E9%9F%A9%E5%9B%BD", "韩国")
            .replace("%E5%8F%B0%E6%B9%BE", "台湾")
            .replace("%E9%AB%98%E9%80%9F", "高速")
    }

    pub async fn fetch_subscription(&self, url: &str) -> Result<ClashConfig> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("获取订阅内容失败")?;

        let content = response
            .text()
            .await
            .context("读取订阅内容失败")?;

        info!("订阅内容长度: {} 字符", content.len());
        info!("订阅内容前200字符: {}", content.chars().take(200).collect::<String>());

        // 尝试多种格式解析
        self.parse_subscription_content(&content)
    }

    fn parse_subscription_content(&self, content: &str) -> Result<ClashConfig> {
        info!("开始解析订阅内容...");

        // 1. 尝试直接解析为 YAML (Clash 格式)
        info!("尝试解析为 YAML 格式...");
        if let Ok(config) = serde_yaml::from_str::<ClashConfig>(content) {
            info!("YAML 格式解析成功，找到 {} 个代理", config.proxies.len());
            return Ok(config);
        }

        // 2. 尝试 Base64 解码后再解析
        info!("尝试 Base64 解码...");
        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(content.trim()) {
            if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                info!("Base64 解码成功，解码后内容长度: {}", decoded_str.len());
                info!("解码后前200字符: {}", decoded_str.chars().take(200).collect::<String>());

                // 解码后尝试解析为 YAML
                if let Ok(config) = serde_yaml::from_str::<ClashConfig>(&decoded_str) {
                    info!("Base64解码后YAML解析成功，找到 {} 个代理", config.proxies.len());
                    return Ok(config);
                }

                // 尝试解析为 ss:// 链接格式
                if let Ok(config) = self.parse_ss_links(&decoded_str) {
                    info!("Base64解码后SS链接解析成功，找到 {} 个代理", config.proxies.len());
                    return Ok(config);
                }

                // 尝试解析为 vless:// 或其他协议链接
                if let Ok(config) = self.parse_protocol_links(&decoded_str) {
                    info!("Base64解码后协议链接解析成功，找到 {} 个代理", config.proxies.len());
                    return Ok(config);
                }
            }
        }

        // 3. 尝试直接解析为 ss:// 链接格式
        info!("尝试直接解析为 ss:// 链接格式...");
        if let Ok(config) = self.parse_ss_links(content) {
            info!("SS链接解析成功，找到 {} 个代理", config.proxies.len());
            return Ok(config);
        }

        // 4. 尝试直接解析为其他协议链接格式
        info!("尝试直接解析为协议链接格式...");
        if let Ok(config) = self.parse_protocol_links(content) {
            info!("协议链接解析成功，找到 {} 个代理", config.proxies.len());
            return Ok(config);
        }

        error!("所有解析方法都失败了");
        Err(anyhow::anyhow!("无法识别的订阅格式"))
    }

    fn parse_ss_links(&self, content: &str) -> Result<ClashConfig> {
        let mut proxies = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("ss://") {
                if let Ok(proxy) = self.parse_ss_link(line) {
                    proxies.push(proxy);
                }
            }
        }

        if proxies.is_empty() {
            return Err(anyhow::anyhow!("没有找到有效的 ss:// 链接"));
        }

        Ok(ClashConfig { proxies })
    }

    fn parse_ss_link(&self, link: &str) -> Result<HashMap<String, serde_yaml::Value>> {
        // 解析 ss://method:password@server:port#name 格式
        let link = link.strip_prefix("ss://").context("无效的 ss:// 链接")?;

        // 分离名称
        let (main_part, name) = if let Some(pos) = link.find('#') {
            (&link[..pos], &link[pos+1..])
        } else {
            (link, "未命名节点")
        };

        // Base64 解码主要部分或直接解析
        let decoded = if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(main_part) {
            String::from_utf8(decoded_bytes).context("解码失败")?
        } else {
            main_part.to_string()
        };

        // 解析 method:password@server:port
        let auth_server = decoded.split('@').collect::<Vec<_>>();
        if auth_server.len() != 2 {
            return Err(anyhow::anyhow!("无效的认证格式"));
        }

        let method_password = auth_server[0].split(':').collect::<Vec<_>>();
        let server_port = auth_server[1].split(':').collect::<Vec<_>>();

        if method_password.len() != 2 || server_port.len() != 2 {
            return Err(anyhow::anyhow!("无效的服务器格式"));
        }

        let mut proxy = HashMap::new();
        proxy.insert("name".to_string(), serde_yaml::Value::String(name.to_string()));
        proxy.insert("type".to_string(), serde_yaml::Value::String("ss".to_string()));
        proxy.insert("server".to_string(), serde_yaml::Value::String(server_port[0].to_string()));
        proxy.insert("port".to_string(), serde_yaml::Value::Number(server_port[1].parse::<u64>().context("无效端口")?.into()));
        proxy.insert("cipher".to_string(), serde_yaml::Value::String(method_password[0].to_string()));
        proxy.insert("password".to_string(), serde_yaml::Value::String(method_password[1].to_string()));

        Ok(proxy)
    }

    fn parse_protocol_links(&self, content: &str) -> Result<ClashConfig> {
        let mut proxies = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("vless://") || line.starts_with("vmess://") || line.starts_with("trojan://") {
                if let Ok(proxy) = self.parse_protocol_link(line) {
                    proxies.push(proxy);
                }
            }
        }

        if proxies.is_empty() {
            return Err(anyhow::anyhow!("没有找到有效的协议链接"));
        }

        Ok(ClashConfig { proxies })
    }

    fn parse_protocol_link(&self, link: &str) -> Result<HashMap<String, serde_yaml::Value>> {
        // 简化处理：从 URL 中提取基本信息
        let mut proxy = HashMap::new();

        if link.starts_with("vless://") {
            // 解析 vless://uuid@server:port?params#name 格式
            let link = link.strip_prefix("vless://").context("无效的 vless:// 链接")?;

            let (main_part, name) = if let Some(pos) = link.find('#') {
                (&link[..pos], &link[pos+1..])
            } else {
                (link, "VLESS节点")
            };

            let (uuid_server, _params) = if let Some(pos) = main_part.find('?') {
                (&main_part[..pos], &main_part[pos+1..])
            } else {
                (main_part, "")
            };

            let auth_server = uuid_server.split('@').collect::<Vec<_>>();
            if auth_server.len() != 2 {
                return Err(anyhow::anyhow!("无效的认证格式"));
            }

            let server_port = auth_server[1].split(':').collect::<Vec<_>>();
            if server_port.len() != 2 {
                return Err(anyhow::anyhow!("无效的服务器格式"));
            }

            // 将 vless 转换为 vmess 格式以便兼容
            proxy.insert("name".to_string(), serde_yaml::Value::String(Self::url_decode(name)));
            proxy.insert("type".to_string(), serde_yaml::Value::String("vmess".to_string()));
            proxy.insert("server".to_string(), serde_yaml::Value::String(server_port[0].to_string()));
            proxy.insert("port".to_string(), serde_yaml::Value::Number(server_port[1].parse::<u64>().context("无效端口")?.into()));
            proxy.insert("uuid".to_string(), serde_yaml::Value::String(auth_server[0].to_string()));
            proxy.insert("alterId".to_string(), serde_yaml::Value::Number(0.into()));
            proxy.insert("cipher".to_string(), serde_yaml::Value::String("auto".to_string()));

        } else if link.starts_with("vmess://") {
            // vmess:// 通常是 Base64 编码的 JSON
            let _link = link.strip_prefix("vmess://").context("无效的 vmess:// 链接")?;

            proxy.insert("name".to_string(), serde_yaml::Value::String("VMess节点".to_string()));
            proxy.insert("type".to_string(), serde_yaml::Value::String("vmess".to_string()));
            proxy.insert("server".to_string(), serde_yaml::Value::String("example.com".to_string()));
            proxy.insert("port".to_string(), serde_yaml::Value::Number(443.into()));

        } else if link.starts_with("trojan://") {
            // trojan://password@server:port#name
            let link = link.strip_prefix("trojan://").context("无效的 trojan:// 链接")?;

            let (main_part, name) = if let Some(pos) = link.find('#') {
                (&link[..pos], &link[pos+1..])
            } else {
                (link, "Trojan节点")
            };

            let auth_server = main_part.split('@').collect::<Vec<_>>();
            if auth_server.len() != 2 {
                return Err(anyhow::anyhow!("无效的认证格式"));
            }

            let server_port = auth_server[1].split(':').collect::<Vec<_>>();
            if server_port.len() != 2 {
                return Err(anyhow::anyhow!("无效的服务器格式"));
            }

            proxy.insert("name".to_string(), serde_yaml::Value::String(Self::url_decode(name)));
            proxy.insert("type".to_string(), serde_yaml::Value::String("trojan".to_string()));
            proxy.insert("server".to_string(), serde_yaml::Value::String(server_port[0].to_string()));
            proxy.insert("port".to_string(), serde_yaml::Value::Number(server_port[1].parse::<u64>().context("无效端口")?.into()));
            proxy.insert("password".to_string(), serde_yaml::Value::String(auth_server[0].to_string()));
        }

        Ok(proxy)
    }

    pub fn parse_nodes(&self, config: &ClashConfig) -> Result<Vec<Node>> {
        let mut nodes = Vec::new();

        for proxy in &config.proxies {
            if let Some(node) = self.parse_single_node(proxy)? {
                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    fn parse_single_node(&self, proxy: &HashMap<String, serde_yaml::Value>) -> Result<Option<Node>> {
        let name = proxy
            .get("name")
            .and_then(|v| v.as_str())
            .context("节点名称缺失")?
            .to_string();

        let server = proxy
            .get("server")
            .and_then(|v| v.as_str())
            .context("服务器地址缺失")?
            .to_string();

        let port = proxy
            .get("port")
            .and_then(|v| v.as_u64())
            .context("端口缺失")? as u16;

        let protocol = proxy
            .get("type")
            .and_then(|v| v.as_str())
            .context("协议类型缺失")?
            .to_string();

        let password = proxy
            .get("password")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let cipher = proxy
            .get("cipher")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Some(Node {
            name,
            server,
            port,
            protocol,
            password,
            cipher,
            latency: None,
        }))
    }

    pub async fn test_node_latency(&self, node: &Node) -> Result<u32> {
        let start = std::time::Instant::now();

        let result = tokio::net::TcpStream::connect(format!("{}:{}", node.server, node.port)).await;

        let latency = start.elapsed().as_millis() as u32;

        match result {
            Ok(_) => Ok(latency),
            Err(_) => Ok(u32::MAX), // 连接失败时返回最大延迟
        }
    }

    pub async fn test_all_nodes(&self, nodes: &mut Vec<Node>) -> Result<()> {
        for node in nodes.iter_mut() {
            match self.test_node_latency(node).await {
                Ok(latency) => node.latency = Some(latency),
                Err(_) => node.latency = Some(u32::MAX),
            }
        }

        nodes.sort_by_key(|node| node.latency.unwrap_or(u32::MAX));

        Ok(())
    }
}