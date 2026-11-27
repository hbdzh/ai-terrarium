<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium Icon" width="88" height="88" />
  <h1>AI Terrarium</h1>
  <p>A cross-platform multi-agent autonomous evolution and observation app built with Tauri v2, Vue 3, and Rust.</p>
  <p>
    <a href="https://github.com/hbdzh/ai-terrarium/releases/tag/v0.1.0">Download v0.1.0</a> ·
    <a href="#key-features">Key Features</a> ·
    <a href="#additional-features">Additional Features</a> ·
    <a href="#getting-started">Build from Source</a> ·
    <a href="mcp/README.md">MCP &amp; Local API</a> ·
    <a href="README.md">简体中文</a>
  </p>
</div>

---

**AI Terrarium** provides an observable, controllable sandbox for multi-agent LLM interactions. You can configure characters with independent models, personas, and reasoning depths, let them evolve autonomously in their own contexts, or step in as an observer and director to inject events, send private hints, or enable Blind Guess Mode.

<a id="key-features"></a>

## Key Features

- **Autonomous Simulation &amp; Director Controls**
  - Supports continuous autonomous turns, single-step progression, and manually specifying the next speaker.
  - Inject public scene events, send private hints (whispers) to individual agents, interject as a bystander/danmaku, or open time-limited private rooms.
  - Includes **Blind Guess Mode** to hide agent reasoning processes and private room messages from the observer view.
- **Information Isolation &amp; Memory Management**
  - Strictly isolates context boundaries between public chats, private rooms, and directed hints so non-participating agents cannot access private content.
  - Supports per-agent **Private Notes** and a **Shared Markdown Board** with automatic anti-leak review or manual approval.
  - Automatically compacts long histories into structured summaries distinguishing confirmed facts, character claims, and open threads.
- **Scheduling &amp; Auxiliary Model Routing**
  - Supports context-aware speaker selection, optional turn passing (`[PASS]`), concise fast replies, random mood variations, and per-agent reasoning depth.
  - Allows routing high-frequency tasks—such as speaker selection and shared board review—to a dedicated lightweight auxiliary model to balance speed and cost.

<a id="additional-features"></a>

## Additional Features

- **Multi-Provider Support &amp; Decoupled Profiles**: Supports custom OpenAI-compatible endpoints with built-in presets for DeepSeek, Alibaba Cloud Bailian (Qwen), Moonshot (Kimi), Volcengine (Doubao), OpenAI, Google Gemini, Zhipu (GLM), MiniMax, and OpenRouter. Model connection profiles are managed separately from character personas.
- **Experiment Forking &amp; Comparison**: Fork an existing experiment and adjust parameters or personas to compare outcomes across runs.
- **Observer Reports &amp; Twist Cards**: Generate periodic observer analysis reports and draw twist event cards to inject into the active scene.
- **Character Library, Tags &amp; Script View**: Includes reusable character presets, experiment style tags, scenario import/export, and a clean script-reading/print view.
- **UI &amp; Rendering**: Collapsible reasoning blocks, code syntax highlighting, KaTeX math rendering, and Light / Dark / System theme support.
- **Local Storage &amp; External Integration**: Persists data locally in SQLite; includes an opt-in local HTTP / WebSocket API bound to `127.0.0.1` and a zero-dependency Node.js MCP bridge ([mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)) for Claude Desktop or Cursor.

## Downloads &amp; Platform Support

Latest release: **[v0.1.0](https://github.com/hbdzh/ai-terrarium/releases/tag/v0.1.0)**

| Platform | Download / Status | Notes |
| --- | --- | --- |
| Windows (x64) | [Portable single exe](https://github.com/hbdzh/ai-terrarium/releases/download/v0.1.0/ai-terrarium-v0.1.0-windows-x64.exe) | Standalone executable; unsigned (choose "Run anyway" if SmartScreen prompts) |
| Android (arm64) | [Download APK](https://github.com/hbdzh/ai-terrarium/releases/download/v0.1.0/ai-terrarium-v0.1.0-android-arm64.apk) | `arm64-v8a` only; currently debug-signed |
| macOS | Build from source | Native overlay titlebar layout |
| HarmonyOS / OpenHarmony | Build from source | Adaptation completed (`ohos_compat.rs` and `feat/open-harmony` branch patches) |

<a id="getting-started"></a>

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/) toolchain &amp; Cargo
- [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)

### Build &amp; Run

```bash
# Install frontend dependencies
npm install

# Launch Tauri in development mode
npm run tauri dev

# Build production bundle
npm run tauri build
```

Additional checks:

```bash
# Frontend typecheck and production bundle
npm run build

# Rust check and unit tests (run inside src-tauri/)
cargo check
cargo test
```

## Documentation

- [Local HTTP / WebSocket API &amp; MCP Bridge](mcp/README.md) (Chinese)

## References &amp; Acknowledgements

This project draws architectural and workflow inspiration in multi-agent orchestration, plugin/tool design, and context management from:

- [openai/codex](https://github.com/openai/codex) — Lightweight coding agent that runs in your terminal
- [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) — DeepSeek Harness: Everything is a Plugin

## License

No `LICENSE` file is currently included in this repository. Please check with the maintainer before redistributing.
