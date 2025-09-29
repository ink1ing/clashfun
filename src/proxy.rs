use anyhow::{Context, Result};
use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{RwLock, Mutex};
use std::collections::HashMap;
use std::time::Duration;

use crate::subscription::Node;

pub struct ProxyServer {
    port: u16,
    current_node: Arc<RwLock<Option<Node>>>,
    udp_sessions: Arc<Mutex<HashMap<SocketAddr, Arc<UdpSocket>>>>,
    is_running: Arc<RwLock<bool>>,
}

impl ProxyServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            current_node: Arc::new(RwLock::new(None)),
            udp_sessions: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn set_node(&self, node: Node) {
        let mut current = self.current_node.write().await;
        *current = Some(node);
        info!("代理节点已切换");
    }

    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        *running = false;
        info!("代理服务器停止信号已发送");
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(anyhow::anyhow!("代理服务器已在运行"));
            }
            *running = true;
        }

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
            let is_running = Arc::clone(&self.is_running);
            tokio::spawn(async move {
                loop {
                    if !*is_running.read().await {
                        info!("TCP 服务器收到停止信号");
                        break;
                    }

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
            let udp_sessions = Arc::clone(&self.udp_sessions);
            let is_running = Arc::clone(&self.is_running);
            tokio::spawn(async move {
                let mut buf = [0; 65536];
                loop {
                    if !*is_running.read().await {
                        info!("UDP 服务器收到停止信号");
                        break;
                    }

                    match tokio::time::timeout(Duration::from_millis(100), udp_socket.recv_from(&mut buf)).await {
                        Ok(Ok((size, addr))) => {
                            let node = Arc::clone(&current_node);
                            let socket = Arc::clone(&udp_socket);
                            let sessions = Arc::clone(&udp_sessions);
                            let data = buf[..size].to_vec();

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_udp_packet(socket, data, addr, node, sessions).await {
                                    error!("UDP 包处理错误: {}", e);
                                }
                            });
                        }
                        Ok(Err(e)) => {
                            error!("UDP 接收错误: {}", e);
                            break;
                        }
                        Err(_) => {
                            // 超时，继续循环检查停止信号
                            continue;
                        }
                    }
                }
            })
        };

        tokio::try_join!(tcp_handle, udp_handle)?;

        Ok(())
    }

    async fn handle_tcp_connection(
        client_stream: TcpStream,
        client_addr: SocketAddr,
        current_node: Arc<RwLock<Option<Node>>>,
    ) -> Result<()> {
        info!("新的 TCP 连接来自: {}", client_addr);

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

        // 连接到目标节点
        match TcpStream::connect(format!("{}:{}", node.server, node.port)).await {
            Ok(target_stream) => {
                info!("已连接到目标节点 {}:{}", node.server, node.port);

                // 双向数据转发
                let (mut client_read, mut client_write) = client_stream.into_split();
                let (mut target_read, mut target_write) = target_stream.into_split();

                let client_to_target = async {
                    tokio::io::copy(&mut client_read, &mut target_write).await
                };
                let target_to_client = async {
                    tokio::io::copy(&mut target_read, &mut client_write).await
                };

                if let Err(e) = tokio::try_join!(client_to_target, target_to_client) {
                    warn!("TCP 转发错误: {}", e);
                }

                info!("TCP 连接已关闭: {}", client_addr);
            }
            Err(e) => {
                error!("无法连接到节点 {}:{}: {}", node.server, node.port, e);
            }
        }

        Ok(())
    }

    async fn handle_udp_packet(
        client_socket: Arc<UdpSocket>,
        data: Vec<u8>,
        client_addr: SocketAddr,
        current_node: Arc<RwLock<Option<Node>>>,
        udp_sessions: Arc<Mutex<HashMap<SocketAddr, Arc<UdpSocket>>>>,
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

        info!("通过节点 {} 代理 UDP 包从 {}", node.name, client_addr);

        // 获取或创建到目标节点的 UDP socket
        let target_socket = {
            let mut sessions = udp_sessions.lock().await;
            if let Some(socket) = sessions.get(&client_addr) {
                Arc::clone(socket)
            } else {
                // 创建新的 UDP socket 连接到目标节点
                match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(socket) => {
                        let socket = Arc::new(socket);

                        // 连接到目标节点
                        if let Err(e) = socket.connect(format!("{}:{}", node.server, node.port)).await {
                            error!("无法连接到 UDP 节点 {}:{}: {}", node.server, node.port, e);
                            return Ok(());
                        }

                        sessions.insert(client_addr, Arc::clone(&socket));

                        // 启动反向数据转发任务
                        let client_sock = Arc::clone(&client_socket);
                        let target_sock = Arc::clone(&socket);
                        let sessions_cleanup = Arc::clone(&udp_sessions);
                        tokio::spawn(async move {
                            let mut buf = [0; 65536];
                            loop {
                                match target_sock.recv(&mut buf).await {
                                    Ok(size) => {
                                        if let Err(e) = client_sock.send_to(&buf[..size], client_addr).await {
                                            error!("UDP 反向转发失败: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("UDP 目标接收错误: {}", e);
                                        break;
                                    }
                                }
                            }
                            // 清理会话
                            let mut sessions = sessions_cleanup.lock().await;
                            sessions.remove(&client_addr);
                        });

                        socket
                    }
                    Err(e) => {
                        error!("无法创建 UDP socket: {}", e);
                        return Ok(());
                    }
                }
            }
        };

        // 转发数据到目标节点
        if let Err(e) = target_socket.send(&data).await {
            error!("UDP 转发失败: {}", e);
        }

        Ok(())
    }

    pub fn get_proxy_port(&self) -> u16 {
        self.port
    }
}