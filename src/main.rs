use clap::Parser;
use env_logger;
use log::{error, info};
use std::process;

mod cli;
mod config;
mod game_detect;
mod proxy;
mod subscription;

use cli::Cli;

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
            // TODO: 实现启动逻辑
            println!("🎮 ClashFun 服务已启动");
            Ok(())
        }
        cli::Commands::Stop => {
            info!("停止 ClashFun 服务...");
            // TODO: 实现停止逻辑
            println!("🛑 ClashFun 服务已停止");
            Ok(())
        }
        cli::Commands::Status => {
            info!("检查服务状态...");
            // TODO: 实现状态检查逻辑
            println!("📊 ClashFun 状态: 未运行");
            Ok(())
        }
        cli::Commands::Nodes => {
            info!("获取节点列表...");
            // TODO: 实现节点列表逻辑
            println!("🌐 节点列表:");
            println!("  暂无可用节点，请先设置订阅链接");
            Ok(())
        }
        cli::Commands::SetSubscription { url } => {
            info!("设置订阅链接: {}", url);
            // TODO: 实现设置订阅逻辑
            println!("✅ 订阅链接已设置");
            Ok(())
        }
        cli::Commands::SelectNode { name } => {
            info!("切换到节点: {}", name);
            // TODO: 实现节点切换逻辑
            println!("🔄 已切换到节点: {}", name);
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
    }
}