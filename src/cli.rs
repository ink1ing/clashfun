use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(name = "clashfun")]
#[command(about = "轻量级游戏加速器")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "启动加速服务")]
    Start,

    #[command(about = "停止加速服务")]
    Stop,

    #[command(about = "查看服务状态")]
    Status,

    #[command(about = "列出所有节点")]
    Nodes,

    #[command(about = "设置订阅链接")]
    SetSubscription {
        #[arg(help = "订阅链接 URL")]
        url: String,
    },

    #[command(about = "切换到指定节点")]
    SelectNode {
        #[arg(help = "节点名称")]
        name: String,
    },

    #[command(about = "更新到最新版本")]
    Update,

    #[command(about = "卸载程序")]
    Uninstall,
}