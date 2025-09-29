use anyhow::{Context, Result};
use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::RwLock;

use crate::subscription::Node;

pub struct ProxyServer {
    port: u16,
    current_node: Arc<RwLock<Option<Node>>>,
}

impl ProxyServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            current_node: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_node(&self, node: Node) {
        let mut current = self.current_node.write().await;
        *current = Some(node);
        info!("代理节点已切换");
    }

    pub async fn start(&self) -> Result<()> {
        let tcp_listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))
            .await
            .with_context(|| format!("无法绑定 TCP 端口 {}", self.port))?;

        let udp_socket = Arc::new(
            UdpSocket::bind(format!("127.0.0.1:{}", self.port))
                .await
                .with_context(|| format!("无法绑定 UDP 端口 {}", self.port))?,
        );

        info!("代理服务器启动在端口 {}", self.port);

        let tcp_handle = {
            let current_node = Arc::clone(&self.current_node);
            tokio::spawn(async move {
                loop {
                    match tcp_listener.accept().await {
                        Ok((stream, addr)) => {
                            let node = Arc::clone(&current_node);
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_tcp_connection(stream, addr, node).await {
                                    error!("TCP 连接处理错误: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("TCP 监听错误: {}", e);
                            break;
                        }
                    }
                }
            })
        };

        let udp_handle = {
            let current_node = Arc::clone(&self.current_node);
            let udp_socket = Arc::clone(&udp_socket);
            tokio::spawn(async move {
                let mut buf = [0; 65536];
                loop {
                    match udp_socket.recv_from(&mut buf).await {
                        Ok((size, addr)) => {
                            let node = Arc::clone(&current_node);
                            let socket = Arc::clone(&udp_socket);
                            let data = buf[..size].to_vec();

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_udp_packet(socket, data, addr, node).await {
                                    error!("UDP 包处理错误: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("UDP 接收错误: {}", e);
                            break;
                        }
                    }
                }
            })
        };

        tokio::try_join!(tcp_handle, udp_handle)?;

        Ok(())
    }

    async fn handle_tcp_connection(
        _stream: TcpStream,
        addr: SocketAddr,
        current_node: Arc<RwLock<Option<Node>>>,
    ) -> Result<()> {
        info!("新的 TCP 连接来自: {}", addr);

        let node = {
            let guard = current_node.read().await;
            match guard.as_ref() {
                Some(node) => node.clone(),
                None => {
                    warn!("没有可用的代理节点");
                    return Ok(());
                }
            }
        };

        info!("通过节点 {} 代理 TCP 连接", node.name);

        Ok(())
    }

    async fn handle_udp_packet(
        _socket: Arc<UdpSocket>,
        _data: Vec<u8>,
        client_addr: SocketAddr,
        current_node: Arc<RwLock<Option<Node>>>,
    ) -> Result<()> {
        let node = {
            let guard = current_node.read().await;
            match guard.as_ref() {
                Some(node) => node.clone(),
                None => {
                    warn!("没有可用的代理节点");
                    return Ok(());
                }
            }
        };

        info!("通过节点 {} 代理 UDP 包到 {}", node.name, client_addr);

        Ok(())
    }

    pub fn get_proxy_port(&self) -> u16 {
        self.port
    }
}