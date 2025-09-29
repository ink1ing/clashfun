use anyhow::{Context, Result};
use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{RwLock, Mutex};
use std::collections::HashMap;
use std::time::Duration;

use crate::subscription::{Node, SubscriptionManager};
use crate::game_detect::{GameDetector, SupportedGame};

pub struct ProxyServer {
    port: u16,
    current_node: Arc<RwLock<Option<Node>>>,
    udp_sessions: Arc<Mutex<HashMap<SocketAddr, Arc<UdpSocket>>>>,
    is_running: Arc<RwLock<bool>>,
    game_detector: Arc<Mutex<GameDetector>>,
    backup_nodes: Arc<RwLock<Vec<Node>>>,
    subscription_url: Arc<RwLock<Option<String>>>,
    node_failure_count: Arc<RwLock<HashMap<String, u32>>>,
}

impl ProxyServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            current_node: Arc::new(RwLock::new(None)),
            udp_sessions: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            game_detector: Arc::new(Mutex::new(GameDetector::new())),
            backup_nodes: Arc::new(RwLock::new(Vec::new())),
            subscription_url: Arc::new(RwLock::new(None)),
            node_failure_count: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_node(&self, node: Node) {
        let mut current = self.current_node.write().await;
        *current = Some(node);
        info!("代理节点已切换");
    }

    pub async fn set_subscription_url(&self, url: String) {
        let mut sub_url = self.subscription_url.write().await;
        *sub_url = Some(url);
    }

    pub async fn set_backup_nodes(&self, nodes: Vec<Node>) {
        let mut backup = self.backup_nodes.write().await;
        *backup = nodes;
        info!("设置了 {} 个备用节点", backup.len());
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

        // 启动健康监控
        let current_node_clone = Arc::clone(&self.current_node);
        let is_running_clone = Arc::clone(&self.is_running);
        let failure_count_clone = Arc::clone(&self.node_failure_count);
        let backup_nodes_clone = Arc::clone(&self.backup_nodes);
        let subscription_url_clone = Arc::clone(&self.subscription_url);

        Self::start_health_monitor_task(
            current_node_clone,
            is_running_clone,
            failure_count_clone,
            backup_nodes_clone,
            subscription_url_clone
        ).await;

        let tcp_handle = {
            let current_node = Arc::clone(&self.current_node);
            let is_running = Arc::clone(&self.is_running);
            let game_detector = Arc::clone(&self.game_detector);
            tokio::spawn(async move {
                loop {
                    if !*is_running.read().await {
                        info!("TCP 服务器收到停止信号");
                        break;
                    }

                    match tcp_listener.accept().await {
                        Ok((stream, addr)) => {
                            let node = Arc::clone(&current_node);
                            let detector = Arc::clone(&game_detector);
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_tcp_connection(stream, addr, node, detector).await {
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
            let game_detector = Arc::clone(&self.game_detector);
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

                            let detector = Arc::clone(&game_detector);
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_udp_packet(socket, data, addr, node, sessions, detector).await {
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
        game_detector: Arc<Mutex<GameDetector>>,
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

        // 检测游戏流量
        let mut _detected_game = None;
        {
            let mut detector = game_detector.lock().await;
            if let Ok(detected_games) = detector.detect_running_games() {
                for (game, _) in detected_games {
                    let game_ports = game.get_game_ports();
                    if game_ports.contains(&client_addr.port()) {
                        info!("检测到游戏 {} 的 TCP 流量 (端口: {})", game.display_name(), client_addr.port());
                        _detected_game = Some(game);
                        break;
                    }
                }
            }
        }

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
        game_detector: Arc<Mutex<GameDetector>>,
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

        // 检测游戏流量
        let mut detected_game = None;
        {
            let mut detector = game_detector.lock().await;
            if let Ok(detected_games) = detector.detect_running_games() {
                for (game, _) in detected_games {
                    let game_ports = game.get_game_ports();

                    // 检查端口匹配
                    if game_ports.contains(&client_addr.port()) {
                        info!("检测到游戏 {} 的 UDP 流量 (端口: {})", game.display_name(), client_addr.port());
                        detected_game = Some(game.clone());
                        break;
                    }

                    // 检查数据包特征
                    if Self::is_game_packet_static(&game, &data) {
                        info!("检测到游戏 {} 的 UDP 数据包特征", game.display_name());
                        detected_game = Some(game.clone());
                        break;
                    }
                }
            }
        }

        info!("通过节点 {} 代理 UDP 包从 {}", node.name, client_addr);
        if let Some(ref game) = detected_game {
            info!("使用游戏 {} 的优化配置", game.display_name());
        }

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

    async fn detect_game_traffic(&self, client_addr: SocketAddr, data: &[u8]) -> Option<SupportedGame> {
        let mut detector = self.game_detector.lock().await;

        if let Ok(detected_games) = detector.detect_running_games() {
            for (game, _) in detected_games {
                let game_ports = game.get_game_ports();

                if game_ports.contains(&client_addr.port()) {
                    info!("检测到游戏 {} 的流量 (端口: {})", game.display_name(), client_addr.port());
                    return Some(game);
                }

                if data.len() > 8 {
                    if self.is_game_packet(&game, data) {
                        info!("检测到游戏 {} 的数据包特征", game.display_name());
                        return Some(game);
                    }
                }
            }
        }

        None
    }

    fn is_game_packet(&self, game: &SupportedGame, data: &[u8]) -> bool {
        match game {
            SupportedGame::DontStarveTogether => {
                data.starts_with(b"KU_") ||
                data.windows(4).any(|w| w == &[0x04, 0x00, 0x00, 0x00]) ||
                data.len() > 20 && data[0] == 0x04
            },
            SupportedGame::CounterStrike => {
                data.starts_with(b"Source Engine Query") ||
                data.windows(4).any(|w| w == &[0xFF, 0xFF, 0xFF, 0xFF]) ||
                (data.len() > 4 && data[0..4] == [0xFF, 0xFF, 0xFF, 0xFF])
            },
            SupportedGame::Dota2 => {
                data.starts_with(b"Source Engine Query") ||
                data.windows(4).any(|w| w == &[0x56, 0x44, 0x50, 0x00]) ||
                data.len() > 8 && data[4] == 0x56
            },
            SupportedGame::LeagueOfLegends => {
                data.len() > 10 && (
                    data.starts_with(&[0x00, 0x0C]) ||
                    data.windows(4).any(|w| w == &[0x17, 0x00, 0x00, 0x00]) ||
                    data[2] == 0x00 && data[3] == 0x01
                )
            },
            SupportedGame::Valorant => {
                data.len() > 12 && (
                    data.starts_with(&[0x00, 0x10]) ||
                    data.windows(4).any(|w| w == &[0x52, 0x69, 0x6F, 0x74]) || // "Riot"
                    data[0] == 0x17 && data[1] == 0x03
                )
            },
            SupportedGame::Minecraft => {
                data.len() > 6 && (
                    data.starts_with(&[0xFE, 0x01]) ||
                    data.starts_with(&[0x00, 0x00]) ||
                    (data[0] >= 0x00 && data[0] <= 0x7F && data[1] == 0x00)
                )
            },
            SupportedGame::ApexLegends => {
                data.starts_with(b"Source Engine Query") ||
                data.windows(4).any(|w| w == &[0x4F, 0x52, 0x49, 0x47]) || // "ORIG"
                data.len() > 16 && data[8] == 0x52
            },
            SupportedGame::Overwatch => {
                data.len() > 8 && (
                    data.starts_with(&[0x42, 0x4E, 0x45, 0x54]) || // "BNET"
                    data.windows(5).any(|w| w == &[0x01, 0x00, 0x00, 0x00, 0x02]) ||
                    data[0] == 0x17 && data[4] == 0x01
                )
            },
        }
    }

    fn should_optimize_for_game(&self, game: &SupportedGame) -> bool {
        game.should_optimize()
    }

    fn get_game_specific_timeout(&self, game: &SupportedGame) -> Duration {
        match game {
            SupportedGame::DontStarveTogether => Duration::from_millis(50),
            SupportedGame::CounterStrike | SupportedGame::Dota2 | SupportedGame::Valorant => {
                Duration::from_millis(20)
            },
            SupportedGame::LeagueOfLegends => Duration::from_millis(30),
            SupportedGame::Minecraft => Duration::from_millis(100),
            SupportedGame::ApexLegends | SupportedGame::Overwatch => Duration::from_millis(25),
        }
    }

    fn is_game_packet_static(game: &SupportedGame, data: &[u8]) -> bool {
        match game {
            SupportedGame::DontStarveTogether => {
                data.starts_with(b"KU_") ||
                data.windows(4).any(|w| w == &[0x04, 0x00, 0x00, 0x00]) ||
                data.len() > 20 && data[0] == 0x04
            },
            SupportedGame::CounterStrike => {
                data.starts_with(b"Source Engine Query") ||
                data.windows(4).any(|w| w == &[0xFF, 0xFF, 0xFF, 0xFF]) ||
                (data.len() > 4 && data[0..4] == [0xFF, 0xFF, 0xFF, 0xFF])
            },
            SupportedGame::Dota2 => {
                data.starts_with(b"Source Engine Query") ||
                data.windows(4).any(|w| w == &[0x56, 0x44, 0x50, 0x00]) ||
                data.len() > 8 && data[4] == 0x56
            },
            SupportedGame::LeagueOfLegends => {
                data.len() > 10 && (
                    data.starts_with(&[0x00, 0x0C]) ||
                    data.windows(4).any(|w| w == &[0x17, 0x00, 0x00, 0x00]) ||
                    data[2] == 0x00 && data[3] == 0x01
                )
            },
            SupportedGame::Valorant => {
                data.len() > 12 && (
                    data.starts_with(&[0x00, 0x10]) ||
                    data.windows(4).any(|w| w == &[0x52, 0x69, 0x6F, 0x74]) || // "Riot"
                    data[0] == 0x17 && data[1] == 0x03
                )
            },
            SupportedGame::Minecraft => {
                data.len() > 6 && (
                    data.starts_with(&[0xFE, 0x01]) ||
                    data.starts_with(&[0x00, 0x00]) ||
                    (data[0] >= 0x00 && data[0] <= 0x7F && data[1] == 0x00)
                )
            },
            SupportedGame::ApexLegends => {
                data.starts_with(b"Source Engine Query") ||
                data.windows(4).any(|w| w == &[0x4F, 0x52, 0x49, 0x47]) || // "ORIG"
                data.len() > 16 && data[8] == 0x52
            },
            SupportedGame::Overwatch => {
                data.len() > 8 && (
                    data.starts_with(&[0x42, 0x4E, 0x45, 0x54]) || // "BNET"
                    data.windows(5).any(|w| w == &[0x01, 0x00, 0x00, 0x00, 0x02]) ||
                    data[0] == 0x17 && data[4] == 0x01
                )
            },
        }
    }

    async fn check_node_health(&self, node: &Node) -> bool {
        info!("检查节点健康状态: {}", node.name);

        match tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect(format!("{}:{}", node.server, node.port))
        ).await {
            Ok(Ok(_)) => {
                info!("节点 {} 健康检查通过", node.name);
                true
            }
            Ok(Err(e)) => {
                warn!("节点 {} 连接失败: {}", node.name, e);
                false
            }
            Err(_) => {
                warn!("节点 {} 健康检查超时", node.name);
                false
            }
        }
    }

    async fn record_node_failure(&self, node_name: &str) {
        let mut failure_count = self.node_failure_count.write().await;
        let count = failure_count.entry(node_name.to_string()).or_insert(0);
        *count += 1;
        warn!("节点 {} 故障计数: {}", node_name, count);
    }

    async fn get_node_failure_count(&self, node_name: &str) -> u32 {
        let failure_count = self.node_failure_count.read().await;
        failure_count.get(node_name).copied().unwrap_or(0)
    }

    async fn reset_node_failure_count(&self, node_name: &str) {
        let mut failure_count = self.node_failure_count.write().await;
        failure_count.insert(node_name.to_string(), 0);
        info!("重置节点 {} 故障计数", node_name);
    }

    async fn try_switch_to_backup_node(&self) -> Result<bool> {
        info!("尝试切换到备用节点...");

        let backup_nodes = {
            let nodes = self.backup_nodes.read().await;
            nodes.clone()
        };

        if backup_nodes.is_empty() {
            warn!("没有可用的备用节点");
            return Ok(false);
        }

        // 按延迟排序，选择最优节点
        let mut available_nodes = Vec::new();
        for node in backup_nodes {
            if self.get_node_failure_count(&node.name).await < 3 {
                if self.check_node_health(&node).await {
                    available_nodes.push(node);
                }
            }
        }

        if available_nodes.is_empty() {
            warn!("所有备用节点都不可用");
            return Ok(false);
        }

        // 选择第一个可用节点（已按延迟排序）
        let best_node = available_nodes.into_iter().next().unwrap();
        info!("切换到备用节点: {}", best_node.name);

        self.set_node(best_node).await;
        Ok(true)
    }

    async fn refresh_backup_nodes(&self) -> Result<()> {
        let subscription_url = {
            let url = self.subscription_url.read().await;
            url.clone()
        };

        if let Some(url) = subscription_url {
            info!("刷新备用节点列表...");

            let sub_manager = SubscriptionManager::new();
            match sub_manager.fetch_subscription(&url).await {
                Ok(clash_config) => {
                    match sub_manager.parse_nodes(&clash_config) {
                        Ok(mut nodes) => {
                            // 测试节点延迟并排序
                            if let Err(e) = sub_manager.test_all_nodes(&mut nodes).await {
                                warn!("节点延迟测试失败: {}", e);
                            }

                            // 过滤可用节点（延迟 < 1000ms）
                            let available_nodes: Vec<Node> = nodes
                                .into_iter()
                                .filter(|n| n.latency.unwrap_or(u32::MAX) < 1000)
                                .collect();

                            self.set_backup_nodes(available_nodes).await;
                            info!("备用节点列表已刷新");
                        }
                        Err(e) => {
                            error!("解析备用节点失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("获取订阅内容失败: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn start_health_monitor_task(
        current_node: Arc<RwLock<Option<Node>>>,
        is_running: Arc<RwLock<bool>>,
        failure_count: Arc<RwLock<HashMap<String, u32>>>,
        backup_nodes: Arc<RwLock<Vec<Node>>>,
        subscription_url: Arc<RwLock<Option<String>>>,
    ) {

        tokio::spawn(async move {
            let mut check_interval = tokio::time::interval(Duration::from_secs(30));
            let mut refresh_interval = tokio::time::interval(Duration::from_secs(300)); // 5分钟刷新一次

            loop {
                if !*is_running.read().await {
                    break;
                }

                tokio::select! {
                    _ = check_interval.tick() => {
                        let current = {
                            let node_guard = current_node.read().await;
                            node_guard.clone()
                        };

                        if let Some(node) = current {
                            // 健康检查当前节点
                            let health_check = tokio::time::timeout(
                                Duration::from_secs(5),
                                TcpStream::connect(format!("{}:{}", node.server, node.port))
                            ).await;

                            match health_check {
                                Ok(Ok(_)) => {
                                    // 节点健康，重置故障计数
                                    let mut count = failure_count.write().await;
                                    count.insert(node.name.clone(), 0);
                                }
                                Ok(Err(_)) | Err(_) => {
                                    // 节点故障，增加故障计数
                                    let mut count = failure_count.write().await;
                                    let current_count = count.entry(node.name.clone()).or_insert(0);
                                    *current_count += 1;

                                    warn!("节点 {} 健康检查失败，故障次数: {}", node.name, current_count);

                                    // 如果故障次数达到阈值，尝试切换备用节点
                                    if *current_count >= 3 {
                                        error!("节点 {} 连续故障 {} 次，尝试切换备用节点", node.name, current_count);

                                        let backup = backup_nodes.read().await;
                                        for backup_node in backup.iter() {
                                            let backup_health = tokio::time::timeout(
                                                Duration::from_secs(3),
                                                TcpStream::connect(format!("{}:{}", backup_node.server, backup_node.port))
                                            ).await;

                                            if backup_health.is_ok() && backup_health.unwrap().is_ok() {
                                                info!("切换到备用节点: {}", backup_node.name);
                                                let mut current_guard = current_node.write().await;
                                                *current_guard = Some(backup_node.clone());

                                                // 重置新节点的故障计数
                                                count.insert(backup_node.name.clone(), 0);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ = refresh_interval.tick() => {
                        // 定期刷新备用节点列表
                        if let Some(url) = subscription_url.read().await.clone() {
                            info!("定期刷新备用节点列表...");

                            let sub_manager = SubscriptionManager::new();
                            if let Ok(clash_config) = sub_manager.fetch_subscription(&url).await {
                                if let Ok(mut nodes) = sub_manager.parse_nodes(&clash_config) {
                                    let _ = sub_manager.test_all_nodes(&mut nodes).await;

                                    let available_nodes: Vec<Node> = nodes
                                        .into_iter()
                                        .filter(|n| n.latency.unwrap_or(u32::MAX) < 1000)
                                        .collect();

                                    let mut backup = backup_nodes.write().await;
                                    *backup = available_nodes;
                                    info!("备用节点列表已刷新，共 {} 个可用节点", backup.len());
                                }
                            }
                        }
                    }
                }
            }

            info!("健康监控任务已停止");
        });

        info!("节点健康监控已启动");
    }
}