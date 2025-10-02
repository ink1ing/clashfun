use std::io::{self, Write};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use anyhow::Result;
use crate::{config::Config, subscription::Node, proxy::ProxyServer, game_detect::GameDetector};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct InteractiveApp {
    pub config: Arc<RwLock<Config>>,
    pub proxy_server: Option<Arc<ProxyServer>>,
    pub game_detector: Arc<RwLock<GameDetector>>,
    pub should_quit: bool,
    pub input: String,
    pub status_message: String,
    pub nodes: Vec<Node>,
    pub selected_node: Option<usize>,
    pub list_state: ListState,
    pub current_mode: AppMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Main,
    NodeSelection,
    Help,
}

impl InteractiveApp {
    pub fn new(config: Arc<RwLock<Config>>, game_detector: Arc<RwLock<GameDetector>>) -> Self {
        Self {
            config,
            proxy_server: None,
            game_detector,
            should_quit: false,
            input: String::new(),
            status_message: "欢迎使用 ClashFun! 输入 /help 查看帮助".to_string(),
            nodes: Vec::new(),
            selected_node: None,
            list_state: ListState::default(),
            current_mode: AppMode::Main,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // 设置终端
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // 加载节点
        self.load_nodes().await?;

        // 主循环
        let result = self.run_app(&mut terminal).await;

        // 恢复终端
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                match self.current_mode {
                    AppMode::Main => self.handle_main_input(key).await?,
                    AppMode::NodeSelection => self.handle_node_selection_input(key).await?,
                    AppMode::Help => self.handle_help_input(key).await?,
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // 标题
                Constraint::Min(0),     // 主内容
                Constraint::Length(3),  // 输入框
                Constraint::Length(2),  // 状态栏
            ])
            .split(f.size());

        // 标题
        let title = Paragraph::new("🎮 ClashFun - 轻量级游戏加速器")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // 主内容区域
        match self.current_mode {
            AppMode::Main => self.render_main_content(f, chunks[1]),
            AppMode::NodeSelection => self.render_node_selection(f, chunks[1]),
            AppMode::Help => self.render_help(f, chunks[1]),
        }

        // 输入框
        let input = Paragraph::new(format!("> {}", self.input))
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("命令输入"));
        f.render_widget(input, chunks[2]);

        // 状态栏
        let status = Paragraph::new(self.status_message.clone())
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("状态"));
        f.render_widget(status, chunks[3]);
    }

    fn render_main_content(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // 左侧：服务状态
        let status_text = vec![
            Line::from(vec![
                Span::styled("📊 服务状态: ", Style::default().fg(Color::White)),
                Span::styled(
                    if self.proxy_server.is_some() { "运行中" } else { "未运行" },
                    Style::default().fg(if self.proxy_server.is_some() { Color::Green } else { Color::Red })
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("🌐 当前节点: ", Style::default().fg(Color::White)),
                Span::styled("未选择", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("🚪 代理端口: ", Style::default().fg(Color::White)),
                Span::styled("7890", Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("🎮 检测到游戏: ", Style::default().fg(Color::White)),
                Span::styled("无", Style::default().fg(Color::Gray)),
            ]),
        ];

        let status_block = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL).title("服务信息"))
            .style(Style::default().fg(Color::White));
        f.render_widget(status_block, chunks[0]);

        // 右侧：可用命令
        let commands = vec![
            "🚀 /start    - 启动加速服务",
            "🛑 /stop     - 停止加速服务",
            "📊 /status   - 查看服务状态",
            "🌐 /nodes    - 查看节点列表",
            "🎯 /select   - 选择节点",
            "⚙️  /set     - 设置订阅链接",
            "🔄 /auto     - 自动选择最优节点",
            "🎮 /detect   - 检测运行中的游戏",
            "⬆️  /update   - 检查并更新到最新版本",
            "❓ /help     - 显示帮助信息",
            "🚪 /quit     - 退出程序",
        ];

        let command_items: Vec<ListItem> = commands
            .iter()
            .map(|cmd| ListItem::new(Line::from(*cmd)))
            .collect();

        let commands_list = List::new(command_items)
            .block(Block::default().borders(Borders::ALL).title("可用命令"))
            .style(Style::default().fg(Color::White));
        f.render_widget(commands_list, chunks[1]);
    }

    fn render_node_selection(&mut self, f: &mut Frame, area: Rect) {
        if self.nodes.is_empty() {
            let msg = Paragraph::new("没有可用的节点，请先设置订阅链接 (/set)")
                .block(Block::default().borders(Borders::ALL).title("节点选择"))
                .style(Style::default().fg(Color::Red));
            f.render_widget(msg, area);
            return;
        }

        let items: Vec<ListItem> = self.nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let style = if Some(i) == self.selected_node {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(format!(
                    "{} {} - {}ms",
                    node.name,
                    node.server,
                    node.latency.unwrap_or(999)
                ))).style(style)
            })
            .collect();

        let nodes_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("节点选择 (↑↓选择, Enter确认, Esc返回)"))
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD));

        f.render_stateful_widget(nodes_list, area, &mut self.list_state);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from("🎮 ClashFun 交互式界面帮助"),
            Line::from(""),
            Line::from("📋 主要命令:"),
            Line::from("  /start    - 启动游戏加速服务"),
            Line::from("  /stop     - 停止加速服务"),
            Line::from("  /status   - 查看当前服务状态"),
            Line::from("  /nodes    - 显示所有可用节点"),
            Line::from("  /select   - 进入节点选择界面"),
            Line::from("  /set      - 设置订阅链接"),
            Line::from("  /auto     - 自动选择最优节点"),
            Line::from("  /detect   - 检测运行中的游戏"),
            Line::from("  /update   - 检查并更新到最新版本"),
            Line::from("  /quit     - 退出程序"),
            Line::from(""),
            Line::from("⌨️  快捷键:"),
            Line::from("  Ctrl+C    - 强制退出"),
            Line::from("  Esc       - 返回主界面"),
            Line::from("  ↑↓        - 在选择界面中导航"),
            Line::from("  Enter     - 确认选择"),
            Line::from(""),
            Line::from("💡 提示: 所有命令都以 '/' 开头"),
        ];

        let help_block = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("帮助 (按 Esc 返回)"))
            .style(Style::default().fg(Color::White));
        f.render_widget(help_block, area);
    }

    async fn handle_main_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                let command = self.input.trim().to_string();
                self.input.clear();
                self.execute_command(command).await?;
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_node_selection_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up => {
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.nodes.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
            KeyCode::Down => {
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i >= self.nodes.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    if i < self.nodes.len() {
                        self.selected_node = Some(i);
                        let node = &self.nodes[i];

                        // 更新配置
                        {
                            let mut config = self.config.write().await;
                            config.selected_node = Some(node.name.clone());
                            config.save()?;
                        }

                        self.status_message = format!("✅ 已选择节点: {}", node.name);
                        self.current_mode = AppMode::Main;
                    }
                }
            }
            KeyCode::Esc => {
                self.current_mode = AppMode::Main;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_help_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.current_mode = AppMode::Main;
            }
            _ => {}
        }
        Ok(())
    }

    async fn execute_command(&mut self, command: String) -> Result<()> {
        match command.as_str() {
            "/start" => {
                self.status_message = "🚀 正在启动加速服务...".to_string();
                // TODO: 实现启动逻辑
            }
            "/stop" => {
                self.status_message = "🛑 正在停止加速服务...".to_string();
                // TODO: 实现停止逻辑
            }
            "/status" => {
                self.status_message = "📊 查看服务状态".to_string();
                // TODO: 实现状态查看
            }
            "/nodes" => {
                self.load_nodes().await?;
                self.status_message = format!("🌐 已加载 {} 个节点", self.nodes.len());
            }
            "/select" => {
                if self.nodes.is_empty() {
                    self.status_message = "❌ 没有可用节点，请先设置订阅链接".to_string();
                } else {
                    self.current_mode = AppMode::NodeSelection;
                    self.list_state.select(Some(0));
                    self.status_message = "🎯 使用 ↑↓ 键选择节点，Enter 确认".to_string();
                }
            }
            "/set" => {
                self.status_message = "⚙️ 请在输入框中输入订阅链接".to_string();
                // TODO: 实现订阅链接设置
            }
            "/auto" => {
                self.status_message = "🔄 正在自动选择最优节点...".to_string();
                // TODO: 实现自动选择
            }
            "/detect" => {
                self.status_message = "🎮 正在检测游戏...".to_string();
                // TODO: 实现游戏检测
            }
            "/update" => {
                self.status_message = "🔄 正在检查更新...".to_string();
                self.check_and_update().await?;
            }
            "/help" => {
                self.current_mode = AppMode::Help;
                self.status_message = "❓ 显示帮助信息".to_string();
            }
            "/quit" => {
                self.should_quit = true;
            }
            cmd if cmd.starts_with("/set ") => {
                let url = cmd.strip_prefix("/set ").unwrap().trim();
                self.set_subscription(url.to_string()).await?;
            }
            _ => {
                self.status_message = format!("❌ 未知命令: {}，输入 /help 查看帮助", command);
            }
        }
        Ok(())
    }

    async fn load_nodes(&mut self) -> Result<()> {
        let config = self.config.read().await;
        if let Some(ref url) = config.subscription_url {
            let sub_manager = crate::subscription::SubscriptionManager::new();
            if let Ok(clash_config) = sub_manager.fetch_subscription(url).await {
                if let Ok(mut nodes) = sub_manager.parse_nodes(&clash_config) {
                    // 测试延迟
                    let _ = sub_manager.test_all_nodes(&mut nodes).await;
                    // 按延迟排序
                    nodes.sort_by_key(|node| node.latency.unwrap_or(9999));
                    self.nodes = nodes;
                }
            }
        }
        Ok(())
    }

    async fn set_subscription(&mut self, url: String) -> Result<()> {
        {
            let mut config = self.config.write().await;
            config.subscription_url = Some(url.clone());
            config.save()?;
        }

        self.status_message = format!("✅ 订阅链接已设置: {}", url);
        self.load_nodes().await?;
        Ok(())
    }

    async fn check_and_update(&mut self) -> Result<()> {
        let updater = crate::updater::Updater::new();

        // 检查更新
        match updater.check_for_updates().await {
            Ok(update_info) => {
                if update_info.update_available {
                    self.status_message = format!("🚀 发现新版本 {} -> {}，正在更新...",
                        update_info.current_version,
                        update_info.latest_version.unwrap_or_else(|| "未知".to_string()));

                    if let Some(download_url) = &update_info.download_url {
                        match updater.perform_update(download_url).await {
                            Ok(()) => {
                                self.status_message = "✅ 更新完成！请重启程序".to_string();
                            }
                            Err(e) => {
                                self.status_message = format!("❌ 更新失败: {}", e);
                            }
                        }
                    } else {
                        self.status_message = "❌ 未找到适合的更新文件".to_string();
                    }
                } else {
                    self.status_message = format!("✅ 已是最新版本 {}", update_info.current_version);
                }
            }
            Err(e) => {
                self.status_message = format!("❌ 检查更新失败: {}", e);
            }
        }

        Ok(())
    }
}