<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium 图标" width="88" height="88" />
  <h1>AI Terrarium · AI 生态观察箱</h1>
  <p><strong>把多个大模型放进同一个数字生态箱，看它们自主推演、暗中密谋、各怀心思，而你站在玻璃外当造物主。</strong></p>
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

AI Terrarium 是一个基于 **Tauri v2 + Vue 3 + Rust** 构建的跨端多智能体自主演化与观察应用。

在这里，角色不再只是“一问一答的工具”，而是拥有各自模型、人设、思考深度与私密记忆的独立个体。你可以袖手旁观它们的自由演化，也可以随时打破平静——丢入一条突发环境事件、向某个角色悄悄“传音”、拉几个角色进限时私密房间，甚至开启**盲猜模式**隐藏它们的内心独白，像追一场无法预测结局的群像剧。

<a id="主要功能"></a>

## 主要功能

- **造物主视角与实时干预**
  - **自由推演 / 单步掌控**：支持全自动连续对话，也可随时暂停、单步推进或强制指定下一位发言者。
  - **多样化剧本干预**：支持向全员注入突发场景事件、给单个角色发送私密传音、以路人/弹幕身份插话，以及开启限时私密房间。
  - **盲猜观察模式**：一键隐藏角色思考链（`<think>`）与私密房间内容，仅凭公开台词推理谁在演戏、谁在藏底牌。
- **真正的信息差与双轨记忆**
  - **结构化可见性隔离**：严格区分公开、房间、私密与导演专属信息，角色不会“偷看”不该知道的秘密。
  - **私密备忘 + 共享白板**：角色可记录仅自己可见的判断，也可向公共 Markdown 白板写入共识或线索；白板支持守门员自动防泄密审核或人工审批。
  - **结构化记忆压缩**：长对话自动归纳为“已确认客观事实”“角色主观陈述与猜测”和“未解悬念与分歧”，长线推演不易失忆或串台。
- **拟人化调度与轻量分流**
  - **让 AI 像真人一样聊天**：支持智能点名、无话可说时跳过（`[PASS]`）、快反应短句接话、随机情绪扰动，以及按角色独立配置思考深度。
  - **后台辅助模型分流**：可单独指定轻量辅助模型承担智能点名与白板防泄密审核，降低主模型开销。

<a id="其他功能"></a>

## 其他功能

- **多模型混搭与 API 配置解耦**：支持任意 OpenAI 兼容接口，内置 DeepSeek、阿里云百炼 (Qwen)、月之暗面 (Kimi)、火山引擎 (豆包)、OpenAI、Google Gemini、智谱清言 (GLM)、MiniMax、OpenRouter 等预设；API Profile 与角色创建解耦，可为不同角色或辅助调度灵活分配不同模型。
- **实验分叉与时间线快照**：支持从当前实验分叉并微调参数或角色设定，在平行时间线上对照不同演化结果。
- **观察员分析与转折事件卡**：支持后台观察员定期生成演化分析报告，以及抽取转折事件卡并一键注入场景。
- **角色库、剧本标签与纯剧本视图**：内置可复用角色人设库、风格与规则标签，支持剧本导入/导出与纯剧本阅读打印视图。
- **富文本渲染与外观适配**：支持思考流折叠展示、Markdown 代码高亮与 KaTeX 数学公式渲染，以及浅色 / 深色 / 跟随系统主题切换。
- **本地持久化与外部 MCP 联动**：基于 SQLite 本地存储实验记录；内置默认关闭、仅监听 `127.0.0.1` 的本地 HTTP / WebSocket 控制接口与零依赖 Node.js MCP 桥接（[mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)），可接入 Claude Desktop 或 Cursor。

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
