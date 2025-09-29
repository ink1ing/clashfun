# TODO.md — clash fun 项目任务清单（扩写版）

以下任务按优先级与阶段拆解，涵盖 **开发、测试、文档、运维** 四个方面，确保 clash fun 项目快速迭代且精简。

---

## 阶段一：基础可运行 MVP（最高优先级）

* [x] **项目初始化** (已完成)

  * [x] 创建 GitHub 仓库 [ink1ing/clashfun](https://github.com/ink1ing/clashfun) (本地已完成)
  * [x] 设置 MIT License & README 基础文档 (已完成)
  * [x] 初始化 Rust 项目 (`cargo init --bin clashfun`) (已完成)
  * [x] 配置 `.gitignore`（排除 target/、.idea/、临时配置文件）(已完成)

* [x] **核心功能实现** (基础版已完成)

  * [x] 支持加载订阅链接并解析节点信息 (已完成 - 支持 Clash YAML 和 SS 链接格式)
  * [x] 节点延迟测试（TCP ping / UDP 测速）(已完成 - TCP 连接测试)
  * [x] 基于延迟或丢包率的最优节点选择逻辑 (已完成 - 按延迟排序)
  * [x] 启动本地代理服务并应用节点配置 (已完成 - 框架已建立)
  * [x] 最小 CLI 交互：`set-subscription` `select-node` `nodes` `status` `detect-game` (已完成)

* [x] **全局通用指令实现** (已完成)

  * [x] `cf update` — 从 GitHub release 拉取更新 (基础框架已完成)
  * [x] `cf uninstall` — 卸载二进制和配置 (基础框架已完成)
  * [x] `cf start/stop/restart` — 控制服务 (已完成)
  * [x] `cf set-subscription` — 替换订阅 (已完成)
  * [x] `cf select-node` — 切换节点 (已完成)
  * [x] `cf nodes` — 列出并排序节点 (已完成)
  * [x] `cf auto-select` — 自动选择最优节点 (已完成)
  * [x] `cf force-uninstall` — 一键卸载程序和所有配置 (已完成)
  * [x] `cf reset` — 清除所有配置恢复原始状态 (已完成)

* [ ] **测试**

  * [ ] 单元测试：订阅解析、节点排序
  * [ ] 集成测试：服务启动、切换节点是否生效
  * [ ] 跨平台测试：Linux / macOS / Windows

---

## 阶段二：增强与优化

* [ ] **CLI 体验优化**

  * [ ] 使用 `clap` / `structopt` 实现子命令与参数解析
  * [ ] 丰富输出格式（表格、JSON、纯文本）
  * [ ] 提供交互式节点选择模式（`--interactive`）

* [ ] **节点管理增强**

  * [ ] 节点健康检查与定时刷新
  * [ ] 节点标签与分组（如 `--region jp` `--game valorant`）
  * [ ] 节点测速缓存与过期策略

* [ ] **配置管理**

  * [ ] 默认配置文件路径：`~/.config/clashfun/config.yaml`
  * [ ] 支持多订阅源合并
  * [ ] 提供 `clashfun config edit` 打开默认编辑器

* [ ] **自动化**

  * [ ] 提供 systemd service 文件
  * [ ] 提供 macOS launchctl 配置
  * [ ] Windows 下注册为服务（可选）

---

## 阶段三：运维与发布

* [ ] **CI/CD**

  * [ ] GitHub Actions：构建 & 单元测试
  * [ ] 自动生成 release 二进制（Linux/macOS/Windows）
  * [ ] 生成 Homebrew formula & Linux 包

* [ ] **文档**

  * [ ] 扩写 README：安装、快速开始、示例命令
  * [ ] 生成 API 文档（`cargo doc`）并发布到 GitHub Pages
  * [ ] FAQ 与常见错误排查

* [ ] **社区支持**

  * [ ] Issue 模板 & PR 模板
  * [ ] 贡献指南（CONTRIBUTING.md）
  * [ ] 行为准则（CODE_OF_CONDUCT.md）

---

## 阶段四：未来探索（低优先级）

* [ ] **功能扩展**

  * [ ] 节点智能推荐（机器学习/简单评分模型）
  * [ ] 与游戏库 API 对接，自动识别运行中的游戏并切换节点
  * [ ] 移动端精简版（基于 Rust + cross 编译）

* [ ] **生态整合**

  * [ ] 提供 REST API（供外部调用）
  * [ ] 提供 gRPC 接口（供 GUI 客户端二次开发）

---

✅ **目标**：每个阶段完成后都能发布一个可运行的 release，保证 clash fun 保持极简、稳定、可维护。
