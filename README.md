<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium 图标" width="88" height="88" />
  <h1>AI Terrarium · AI 生态观察箱</h1>
  <p>基于 Tauri v2、Vue 3 与 Rust 构建的跨端多智能体自主演化与观察应用。</p>
  <p>
    <a href="https://github.com/hbdzh/ai-terrarium/releases/tag/v0.1.0">下载 v0.1.0</a> ·
    <a href="#主要功能">主要功能</a> ·
    <a href="#其他功能">其他功能</a> ·
    <a href="#从源码构建">从源码构建</a> ·
    <a href="mcp/README.md">MCP 与本地接口</a> ·
    <a href="README_EN.md">English</a>
  </p>
</div>

---

**AI Terrarium** 为多个大语言模型提供了一个可观察、可干预的沙盒对话空间。你可以为不同角色配置独立的模型、人设与思考深度，让它们在各自语境中自主推演，也可以作为观察者或导演实时注入事件、发送私密提示或开启盲猜模式。

<a id="主要功能"></a>

## 主要功能

- **自主推演与导演干预**
  - 支持连续自动推演、单步推进，以及手动指定下一位发言者。
  - 支持注入全员可见的场景事件、向单个角色发送私密提示、以旁观者/弹幕身份插话，以及开启限时私密房间。
  - 提供**盲猜模式**，可按需隐藏角色思考过程与私密房间内容，保留纯观察视角。
- **信息隔离与记忆管理**
  - 严格隔离公开对话、私密房间与定向提示的上下文边界，确保未参与的角色无法获取私密信息。
  - 支持角色**私密笔记**与**共享 Markdown 白板**；白板写入支持自动防泄密审查或人工确认。
  - 长对话自动进行结构化摘要压缩，区分已确认事实、角色主观表述与未解线索，减少长程推演中的遗忘。
- **调度控制与辅助模型**
  - 支持上下文智能点名、无有效补充时跳过回合（`[PASS]`）、短句快反应回复、随机情绪微调，以及按角色独立设置思考深度。
  - 支持单独指定轻量辅助模型承担智能点名与白板审查等高频任务，兼顾响应速度与调用成本。

<a id="其他功能"></a>

## 其他功能

- **多模型接入与独立配置**：支持兼容 OpenAI 接口的自定义服务，内置 DeepSeek、阿里云百炼 (Qwen)、月之暗面 (Kimi)、火山引擎 (豆包)、OpenAI、Google Gemini、智谱清言 (GLM)、MiniMax、OpenRouter 等常用预设；模型连接配置与角色人设分离管理，便于跨模型组合。
- **实验分叉与对比**：支持从已有实验分叉并调整参数或人设，对照不同设定下的演化走向。
- **观察员分析与事件卡**：支持定期生成观察员分析报告，以及抽取转折事件卡并注入当前场景。
- **角色库、标签与剧本视图**：内置可复用角色库与实验风格标签，支持剧本导入/导出与纯剧本阅读打印视图。
- **界面与渲染**：支持思考过程折叠、代码高亮、KaTeX 数学公式渲染，以及浅色 / 深色 / 跟随系统主题。
- **本地存储与外部接口**：实验数据本地存储于 SQLite；内置默认关闭、仅监听 `127.0.0.1` 的本地 HTTP / WebSocket 接口与零依赖 Node.js MCP 桥接（[mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)），支持接入 Claude Desktop 或 Cursor。

## 下载与支持平台

最新版本：**[v0.1.0](https://github.com/hbdzh/ai-terrarium/releases/tag/v0.1.0)**

| 平台 | 获取方式 | 说明 |
| --- | --- | --- |
| Windows (x64) | [下载单文件 exe](https://github.com/hbdzh/ai-terrarium/releases/download/v0.1.0/ai-terrarium-v0.1.0-windows-x64.exe) | 免安装单文件；未代码签名，首次运行如遇 SmartScreen 提示请选择“仍要运行” |
| Android (arm64) | [下载 APK](https://github.com/hbdzh/ai-terrarium/releases/download/v0.1.0/ai-terrarium-v0.1.0-android-arm64.apk) | 仅包含 `arm64-v8a` 架构，当前为调试签名 |
| macOS | 支持源码构建 | 支持原生标题栏布局与窗口适配 |
| HarmonyOS / OpenHarmony | 支持源码构建 | 已完成适配（`ohos_compat.rs` 与 `feat/open-harmony` 分支依赖） |

<a id="从源码构建"></a>

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

## 参考与致谢

本项目在多智能体调度、工具/插件化架构与上下文管理设计上参考了以下开源项目：

- [openai/codex](https://github.com/openai/codex) — OpenAI 轻量级终端编码代理
- [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) — DeepSeek Harness（Everything is a Plugin）

## 许可证

仓库当前尚未包含 `LICENSE` 文件，如需对外正式分发或二次使用，请联系仓库维护者确认授权方式。
