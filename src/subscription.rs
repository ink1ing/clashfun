use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use base64::{engine::general_purpose, Engine as _};

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

        // 尝试多种格式解析
        self.parse_subscription_content(&content)
    }

    fn parse_subscription_content(&self, content: &str) -> Result<ClashConfig> {
        // 1. 尝试直接解析为 YAML (Clash 格式)
        if let Ok(config) = serde_yaml::from_str::<ClashConfig>(content) {
            return Ok(config);
        }

        // 2. 尝试 Base64 解码后再解析
        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(content.trim()) {
            if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                // 解码后尝试解析为 YAML
                if let Ok(config) = serde_yaml::from_str::<ClashConfig>(&decoded_str) {
                    return Ok(config);
                }

                // 尝试解析为 ss:// 链接格式
                if let Ok(config) = self.parse_ss_links(&decoded_str) {
                    return Ok(config);
                }
            }
        }

        // 3. 尝试直接解析为 ss:// 链接格式
        if let Ok(config) = self.parse_ss_links(content) {
            return Ok(config);
        }

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