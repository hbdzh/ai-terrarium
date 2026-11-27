<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium 图标" width="88" height="88" />
  <h1>AI Terrarium · AI 生态观察箱</h1>
  <p>基于 Tauri v2、Vue 3 与 Rust 构建的桌面/跨端多智能体自主演化与观察应用。</p>
  <p>
    <a href="#快速开始">快速开始</a> ·
    <a href="mcp/README.md">MCP 与本地接口</a> ·
    <a href="https://github.com/hbdzh/ai-terrarium/releases">Releases</a> ·
    <a href="README_EN.md">English</a>
  </p>
</div>

---

AI Terrarium 为多角色大模型对话提供一个可观察、可干预的沙盒空间。你可以配置多个不同模型与人设的智能体，让它们在隔离或共享的语境中自主推演，并以“观察者 / 导演”视角实时介入、记录演化轨迹与分析动态。

## 核心功能

- **多智能体自主推演与导演干预**
  - 支持连续自动推演与单步推进（可指定下一位发言角色）。
  - 支持注入全员可见的突发场景事件、向单个角色发送私密定向提示、以路人/弹幕身份插话，以及开启限时私密房间。
  - 提供**盲猜观察模式**，可按需隐藏角色思考链与私密对话内容，还原纯观察视角。
- **结构化信息隔离与双轨记忆**
  - 基于结构化可见性规则（公开、房间、私密、导演专属）严格隔离不同角色与房间的上下文。
  - 支持**角色私密备忘**与**共享 Markdown 白板**，白板写入支持守门员自动防泄密审核或人工审批。
  - 内置长对话上下文压缩，将历史记录结构化归纳为“已确认客观事实”“角色主观陈述与猜测”与“未解悬念与分歧”。
- **拟人化调度与后台辅助分流**
  - 支持智能点名、允许跳过发言（`[PASS]`）、快反应回复、随机情绪扰动，以及按角色独立配置思考深度。
  - 可选配置独立的**后台辅助调度模型**，将智能点名与守门员审核等高频轻量任务从主模型分流。
- **多模型接入与外部联动**
  - 支持自定义 OpenAI 兼容接口，内置 DeepSeek、阿里云百炼 (Qwen)、月之暗面 (Kimi)、火山引擎 (豆包)、OpenAI、Google Gemini、智谱清言 (GLM)、MiniMax、OpenRouter 等常用预设。
  - 内置默认关闭、仅监听 `127.0.0.1` 的本地 HTTP / WebSocket 接口与零依赖 Node.js MCP 桥接（[mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)），可接入 Claude Desktop 或 Cursor 等客户端。

## 下载与支持平台

当前版本：`v0.1.0`（暂未在 [GitHub Releases](https://github.com/hbdzh/ai-terrarium/releases) 发布预编译安装包，请先通过源码构建运行）。

| 平台 | 状态 | 说明 |
| --- | --- | --- |
| Windows | 支持源码构建 | 已适配自定义窗口外观 |
| macOS | 支持源码构建 | 支持原生标题栏布局与窗口适配 |
| Linux | 支持源码构建 | 标准 Tauri v2 桌面构建 |
| OpenHarmony | 源码适配中 | 仓库包含 `ohos_compat.rs` 及 `feat/open-harmony` 分支依赖适配 |

<a id="快速开始"></a>

## 从源码构建

### 前置条件

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/) 工具链与 Cargo
- [Tauri v2 平台系统依赖](https://v2.tauri.app/start/prerequisites/)

### 安装与运行

```bash
# 安装前端依赖
npm install

# 启动 Tauri 开发模式
npm run tauri dev

# 构建生产安装包
npm run tauri build
```

其他常用校验命令：

```bash
# 前端类型检查与构建
npm run build

# Rust 后端检查与单元测试（在 src-tauri/ 目录执行）
cargo check
cargo test
```

## 进阶文档

- [本地 HTTP / WebSocket 接口与 MCP 桥接说明](mcp/README.md)

## 许可证

仓库当前尚未包含 `LICENSE` 文件，如需对外正式分发或二次使用，请联系仓库维护者确认授权方式。
