# ClashFun 🎮

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)

轻量级游戏加速器，专为游戏优化的网络代理工具。

## 🚀 特性

- 🎯 **游戏专用**：专门为游戏网络优化设计
- 🌐 **订阅支持**：支持 Clash 订阅链接自动解析
- ⚡ **智能选择**：自动测试延迟选择最佳节点
- 🖥️ **跨平台**：支持 macOS、Linux、Windows
- 🎮 **游戏检测**：自动检测运行中的游戏进程
- 📊 **实时监控**：显示连接状态和网络统计

## 📦 安装

### 一键安装
```bash
curl -fsSL https://raw.githubusercontent.com/ink1ing/clashfun/master/install.sh | sh
```

### 手动安装
1. 从 [Releases](https://github.com/ink1ing/clashfun/releases) 下载对应平台的二进制文件
2. 解压并移动到 PATH 目录
3. 赋予执行权限：`chmod +x clashfun`

## 🎯 快速开始

### 1. 设置订阅链接
```bash
cf set-subscription https://your-clash-subscription-url
```

### 2. 查看节点列表
```bash
cf nodes
```

### 3. 启动加速服务
```bash
cf start
```

### 4. 查看状态
```bash
cf status
```

## 📋 命令列表

| 命令 | 描述 |
|------|------|
| `cf start` | 启动加速服务 |
| `cf stop` | 停止加速服务 |
| `cf status` | 查看运行状态 |
| `cf nodes` | 列出所有节点 |
| `cf select-node <name>` | 切换到指定节点 |
| `cf auto-select` | 自动选择最优节点 |
| `cf set-subscription <url>` | 设置订阅链接 |
| `cf detect-game` | 检测运行中的游戏 |
| `cf update` | 更新到最新版本 |
| `cf uninstall` | 卸载程序 |
| `cf force-uninstall` | 一键卸载程序和所有配置 |
| `cf reset` | 清除所有配置恢复原始状态 |

## 🎮 支持的游戏

- Steam《饥荒联机版》(Don't Starve Together)
- 《反恐精英》(Counter-Strike)
- 《刀場2》(Dota2)
- 《英雄联盟》(League of Legends)
- 《无畏契约》(Valorant)
- 《我的世界》(Minecraft)
- 《Apex英雄》(Apex Legends)
- 《守望先锋》(Overwatch)
- 更多游戏支持持续添加中...

## 📁 项目结构

```
clashfun/
├── src/
│   ├── main.rs          # 程序入口
│   ├── cli.rs           # 命令行界面
│   ├── config.rs        # 配置管理
│   ├── subscription.rs  # 订阅解析
│   ├── proxy.rs         # 代理服务
│   └── game_detect.rs   # 游戏检测
├── Cargo.toml           # 项目配置
└── README.md           # 项目说明
```

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

本项目采用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

## 🔗 相关链接

- [项目主页](https://github.com/ink1ing/clashfun)
- [问题反馈](https://github.com/ink1ing/clashfun/issues)
- [发布页面](https://github.com/ink1ing/clashfun/releases)