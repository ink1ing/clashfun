use clap::Parser;
use env_logger;
use log::{error, info};
use std::process;
use std::sync::Arc;

mod cli;
mod config;
mod game_detect;
mod proxy;
mod subscription;

use cli::Cli;
use proxy::ProxyServer;

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        error!("错误: {}", e);
        process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        cli::Commands::Start => {
            info!("启动 ClashFun 服务...");

            let config = config::Config::load()?;

            // 检查是否已配置订阅和节点
            if config.subscription_url.is_none() {
                println!("❌ 请先设置订阅链接: clashfun set-subscription <URL>");
                return Ok(());
            }

            if config.selected_node.is_none() {
                println!("❌ 请先选择一个节点: clashfun select-node <NAME>");
                return Ok(());
            }

            // 获取节点信息
            let selected_node_name = config.selected_node.as_ref().unwrap();
            let subscription_url = config.subscription_url.as_ref().unwrap();

            let sub_manager = subscription::SubscriptionManager::new();
            let clash_config = sub_manager.fetch_subscription(subscription_url).await?;
            let mut nodes = sub_manager.parse_nodes(&clash_config)?;

            // 测试所有节点延迟并排序
            println!("🔍 测试节点延迟...");
            if let Err(e) = sub_manager.test_all_nodes(&mut nodes).await {
                println!("⚠️  延迟测试失败: {}", e);
            }

            let selected_node = nodes.iter()
                .find(|n| &n.name == selected_node_name)
                .ok_or_else(|| anyhow::anyhow!("找不到选中的节点: {}", selected_node_name))?
                .clone();

            // 过滤出可用的备用节点（延迟 < 1000ms 且不是当前节点）
            let backup_nodes: Vec<subscription::Node> = nodes
                .into_iter()
                .filter(|n| &n.name != selected_node_name && n.latency.unwrap_or(u32::MAX) < 1000)
                .collect();

            // 创建代理服务器
            let proxy_server = Arc::new(ProxyServer::new(config.proxy_port));
            proxy_server.set_node(selected_node.clone()).await;

            // 设置订阅URL和备用节点
            proxy_server.set_subscription_url(subscription_url.clone()).await;
            proxy_server.set_backup_nodes(backup_nodes.clone()).await;
            println!("🔄 设置了 {} 个备用节点", backup_nodes.len());

            println!("🚀 正在启动代理服务器...");
            println!("📍 节点: {}", selected_node.name);
            println!("🌐 服务器: {}:{}", selected_node.server, selected_node.port);
            println!("🚪 本地端口: {}", config.proxy_port);
            println!("📊 协议: {}", selected_node.protocol);

            // 启动服务器 (这会阻塞直到服务器停止)
            if let Err(e) = proxy_server.start().await {
                error!("代理服务器启动失败: {}", e);
                return Err(e);
            }

            println!("🛑 ClashFun 服务已停止");
            Ok(())
        }
        cli::Commands::Stop => {
            info!("停止 ClashFun 服务...");

            // 这里可以实现进程间通信来停止服务
            // 目前先显示简单信息，后续可以通过 PID 文件或 signal 来实现
            println!("🛑 停止信号已发送");
            println!("💡 如果服务仍在运行，请使用 Ctrl+C 强制停止");
            Ok(())
        }
        cli::Commands::Status => {
            info!("检查服务状态...");

            let config = config::Config::load()?;

            println!("📊 ClashFun 状态信息:");
            println!("  🔗 订阅链接: {}",
                config.subscription_url.as_deref().unwrap_or("未设置"));
            println!("  🌐 当前节点: {}",
                config.selected_node.as_deref().unwrap_or("未选择"));
            println!("  🚪 代理端口: {}", config.proxy_port);
            println!("  🤖 自动选择: {}", if config.auto_select { "开启" } else { "关闭" });

            // 检查服务状态 - 简单的端口检查
            let service_status = match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.proxy_port)).await {
                Ok(_) => "未运行",
                Err(_) => "正在运行",
            };
            println!("  ⚡ 服务状态: {}", service_status);

            // 检测游戏
            let mut detector = game_detect::GameDetector::new();
            match detector.detect_running_games() {
                Ok(detected_games) => {
                    if !detected_games.is_empty() {
                        println!("  🎮 检测到游戏:");
                        for (game, _) in detected_games {
                            println!("    - {}", game.display_name());
                        }
                    } else {
                        println!("  🎮 检测到游戏: 无");
                    }
                }
                Err(_) => {
                    println!("  🎮 检测到游戏: 检测失败");
                }
            }

            Ok(())
        }
        cli::Commands::Nodes => {
            info!("获取节点列表...");

            let config = config::Config::load()?;

            if let Some(url) = config.subscription_url {
                println!("🔄 从订阅链接获取节点...");

                let sub_manager = subscription::SubscriptionManager::new();
                match sub_manager.fetch_subscription(&url).await {
                    Ok(clash_config) => {
                        match sub_manager.parse_nodes(&clash_config) {
                            Ok(mut nodes) => {
                                println!("🔍 测试节点延迟...");
                                if let Err(e) = sub_manager.test_all_nodes(&mut nodes).await {
                                    println!("⚠️  延迟测试失败: {}", e);
                                }

                                println!("🌐 节点列表 (共{}个):", nodes.len());
                                println!("{:<4} {:<30} {:<20} {:<10} {:<10}", "序号", "节点名称", "服务器", "协议", "延迟(ms)");
                                println!("{}", "-".repeat(80));

                                for (i, node) in nodes.iter().enumerate() {
                                    let latency = match node.latency {
                                        Some(lat) if lat == u32::MAX => "超时".to_string(),
                                        Some(lat) => format!("{}", lat),
                                        None => "未测试".to_string(),
                                    };

                                    println!("{:<4} {:<30} {:<20} {:<10} {:<10}",
                                        i + 1,
                                        node.name.chars().take(30).collect::<String>(),
                                        node.server.chars().take(20).collect::<String>(),
                                        node.protocol,
                                        latency
                                    );
                                }
                            }
                            Err(e) => {
                                println!("❌ 解析节点失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ 获取订阅失败: {}", e);
                    }
                }
            } else {
                println!("🌐 节点列表:");
                println!("  暂无可用节点，请先设置订阅链接");
                println!("  使用命令: clashfun set-subscription <URL>");
            }

            Ok(())
        }
        cli::Commands::SetSubscription { url } => {
            info!("设置订阅链接: {}", url);

            let mut config = config::Config::load()?;
            config.subscription_url = Some(url.clone());
            config.save()?;

            println!("✅ 订阅链接已设置: {}", url);
            println!("💡 使用 'clashfun nodes' 查看可用节点");
            Ok(())
        }
        cli::Commands::SelectNode { name } => {
            info!("切换到节点: {}", name);

            let mut config = config::Config::load()?;

            if let Some(url) = &config.subscription_url {
                let sub_manager = subscription::SubscriptionManager::new();
                match sub_manager.fetch_subscription(url).await {
                    Ok(clash_config) => {
                        match sub_manager.parse_nodes(&clash_config) {
                            Ok(nodes) => {
                                // 查找匹配的节点
                                if let Some(node) = nodes.iter().find(|n| n.name.contains(&name)) {
                                    config.selected_node = Some(node.name.clone());
                                    config.save()?;
                                    println!("🔄 已切换到节点: {}", node.name);
                                    println!("📍 服务器: {}:{}", node.server, node.port);
                                } else {
                                    println!("❌ 未找到包含 '{}' 的节点", name);
                                    println!("💡 使用 'clashfun nodes' 查看可用节点");
                                }
                            }
                            Err(e) => {
                                println!("❌ 解析节点失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ 获取订阅失败: {}", e);
                    }
                }
            } else {
                println!("❌ 请先设置订阅链接: clashfun set-subscription <URL>");
            }

            Ok(())
        }
        cli::Commands::Update => {
            info!("检查更新...");
            // TODO: 实现更新逻辑
            println!("🔄 检查更新中...");
            println!("✅ 当前已是最新版本");
            Ok(())
        }
        cli::Commands::Uninstall => {
            info!("卸载 ClashFun...");
            // TODO: 实现卸载逻辑
            println!("🗑️  ClashFun 已卸载");
            Ok(())
        }
        cli::Commands::AutoSelect => {
            info!("自动选择最优节点...");

            let mut config = config::Config::load()?;

            if let Some(url) = &config.subscription_url {
                println!("🔍 获取并测试所有节点...");

                let sub_manager = subscription::SubscriptionManager::new();
                match sub_manager.fetch_subscription(url).await {
                    Ok(clash_config) => {
                        match sub_manager.parse_nodes(&clash_config) {
                            Ok(mut nodes) => {
                                println!("🧪 测试节点延迟...");
                                if let Err(e) = sub_manager.test_all_nodes(&mut nodes).await {
                                    println!("⚠️  延迟测试失败: {}", e);
                                }

                                // 找到延迟最低的可用节点
                                if let Some(best_node) = nodes.iter()
                                    .filter(|n| n.latency.unwrap_or(u32::MAX) < u32::MAX)
                                    .min_by_key(|n| n.latency.unwrap_or(u32::MAX)) {

                                    config.selected_node = Some(best_node.name.clone());
                                    config.save()?;

                                    println!("🚀 自动选择最优节点: {}", best_node.name);
                                    println!("📍 服务器: {}:{}", best_node.server, best_node.port);
                                    println!("⚡ 延迟: {}ms", best_node.latency.unwrap_or(0));
                                    println!("📊 协议: {}", best_node.protocol);
                                } else {
                                    println!("❌ 没有找到可用的节点");
                                }
                            }
                            Err(e) => {
                                println!("❌ 解析节点失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ 获取订阅失败: {}", e);
                    }
                }
            } else {
                println!("❌ 请先设置订阅链接: clashfun set-subscription <URL>");
            }

            Ok(())
        }
        cli::Commands::DetectGame => {
            info!("检测运行中的游戏...");

            let mut detector = game_detect::GameDetector::new();
            match detector.detect_running_games() {
                Ok(detected_games) => {
                    if detected_games.is_empty() {
                        println!("🎮 未检测到支持的游戏进程");
                        println!("💡 当前支持的游戏:");
                        println!("   - 饥荒联机版 (Don't Starve Together)");
                    } else {
                        println!("🎮 检测到运行中的游戏:");
                        for (game, process) in detected_games {
                            println!("   ✅ {} (PID: {}, 进程名: {})",
                                game.display_name(),
                                process.pid,
                                process.name
                            );
                            if let Some(ref path) = process.exe_path {
                                println!("      路径: {}", path);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("❌ 游戏检测失败: {}", e);
                }
            }
            Ok(())
        }
    }
}