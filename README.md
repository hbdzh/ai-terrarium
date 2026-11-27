<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium 图标" width="88" height="88" />
  <h1>AI Terrarium · AI 生态观察箱</h1>
  <p><strong>把几个不同的 AI 放进同一个“观察箱”里自己聊，你站在外面当导演——随时加戏、发小纸条、拉小黑屋，看它们演成什么样。</strong></p>
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

平时用 AI，大多是你问一句、它答一句。**AI Terrarium** 想做的是另一件事：你搭好一个场景，选几个不同性格、甚至来自不同厂商的 AI 角色（比如 DeepSeek、Qwen、Kimi、GPT、Gemini 等），让它们自己聊起来。

你可以什么都不做，像看群像剧一样看它们自由发挥；也可以随时伸手干预——突然宣布“停电了”、悄悄给其中一个角色塞张小纸条、把两个角色拉进小黑屋密聊几句，或者把它们的内心独白遮起来，自己猜谁在演戏。

<a id="主要功能"></a>

## 主要功能

- **当导演，随时给 AI 加戏**
  - **自动聊 / 一步步看**：可以让角色们自己一直聊下去，也可以随时暂停、手动点“下一轮”，或者指定下一个由谁开口。
  - **四种加戏方式**：
    - **突发事件**：往大厅丢一条系统消息（比如“门外突然传来敲门声”），所有人都会看到并做出反应。
    - **私密传音**：只对某一个角色说悄悄话，其他角色完全听不到。
    - **路人 / 弹幕插话**：以旁观者身份进场说一句话，看它们怎么接梗。
    - **限时小黑屋**：挑几个角色拉进私密房间单聊几轮，时间到了自动回到大厅，外面的人不知道里面聊了啥。
  - **盲猜模式**：开启后会隐藏角色的内心想法和小黑屋内容，只看公开发言，适合玩推理、狼人杀或“猜猜谁是卧底”。
- **真的有“信息差”，AI 不会穿帮**
  - **严格的房间隔离**：大厅、小黑屋、私密纸条彼此完全隔开，不在场的人绝对偷看不到。
  - **角色小本本 + 公共白板**：每个角色都能偷偷记自己的想法和计划；大家也可以一起往公共白板上写共识或线索。白板修改支持自动“防泄密”检查，也可以改成由你手动点同意。
  - **聊太久也不容易忘事**：对话变长时，会自动把前面的内容整理成“已经发生的事”“各人怎么说”和“还没解开的悬念”，避免聊到后面前言不搭后语。
- **让 AI 聊天更像真人**
  - **不强行凑字数**：觉得没话接时，角色可以直接跳过这轮；也可以开启“快反应模式”用短句秒回，或者加一点随机心情变化。
  - **小模型帮大忙**：可以把“点名谁接话”“检查白板别泄密”这类高频小活，单独交给一个更便宜、更快的模型去跑。

<a id="其他功能"></a>

## 其他功能

- **多模型混搭**：支持各类 OpenAI 兼容接口，内置 DeepSeek、阿里云百炼 (Qwen)、月之暗面 (Kimi)、火山引擎 (豆包)、OpenAI、Google Gemini、智谱清言 (GLM)、MiniMax、OpenRouter 等常用预设；账号配置和角色人设分开管理，想让哪两个模型同台都能随时换。
- **平行宇宙分叉**：聊到一半想试试另一种走向？可以直接从当前进度分叉出一个新实验，改一下规则或人设对比着看。
- **后台观察员与转折事件卡**：可以安排一个“观察员”定期总结局势，也可以随机抽几张“转折事件卡”直接丢进剧情里。
- **角色库、风格标签与纯剧本模式**：自带可复用的角色人设库和规则标签（如悬疑推理、日常瞎侃、短句快节奏等），支持导入/导出剧本，也能切到干净的“纯剧本”视图方便阅读或打印。
- **阅读体验与外观**：支持折叠查看 AI 的思考过程、代码高亮、数学公式渲染，以及浅色 / 深色 / 跟随系统主题。
- **本地保存与外部联动**：所有记录保存在本地 SQLite 数据库中；另外内置了默认关闭的本地 HTTP / WebSocket 接口和 MCP 桥接（[mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)），可以接进 Claude Desktop 或 Cursor 来遥控观察箱。

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
