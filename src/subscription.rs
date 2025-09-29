use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

        let config: ClashConfig = serde_yaml::from_str(&content)
            .context("解析订阅配置失败")?;

        Ok(config)
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