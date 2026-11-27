<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium Icon" width="88" height="88" />
  <h1>AI Terrarium</h1>
  <p>A cross-platform multi-agent autonomous evolution and observation sandbox built with Tauri v2, Vue 3, and Rust.</p>
  <p>
    <a href="#getting-started">Getting Started</a> ·
    <a href="mcp/README.md">MCP &amp; Local API</a> ·
    <a href="https://github.com/hbdzh/ai-terrarium/releases">Releases</a> ·
    <a href="README.md">简体中文</a>
  </p>
</div>

---

AI Terrarium provides an observable, controllable sandbox for multi-agent LLM interactions. You can configure multiple agents with distinct models and personas, let them evolve autonomously across shared or isolated contexts, and intervene in real time as an observer or director.

## Key Features

- **Autonomous Multi-Agent Simulation & Director Intervention**
  - Run continuous autonomous turns or step turn-by-turn (optionally specifying the next speaker).
  - Inject public scene events, send private whispers to individual agents, interject as a bystander/danmaku, or open time-limited private rooms.
  - Includes a **Blind Guess Mode** that hides reasoning chains and private rooms from the observer view.
- **Structured Visibility Isolation & Dual-Track Memory**
  - Enforces structured visibility boundaries (`Public`, `Room`, `Private`, `DirectorOnly`) across agents and rooms.
  - Supports **Private Notes** per agent and a **Shared Markdown Board** with either automatic guardian anti-leak review or manual approval.
  - Compacts long conversation histories into structured summaries separating confirmed facts, character claims, and open threads.
- **Scheduling & Natural Interaction Controls**
  - Supports smart speaker selection, optional turn passing (`[PASS]`), intuitive fast replies, random mood perturbations, and per-agent thinking depth.
  - Optional **Auxiliary Model Routing** offloads lightweight background tasks such as smart speaker selection and guardian board review.
- **Multi-Provider Support & External Control**
  - Supports OpenAI-compatible endpoints with built-in presets for DeepSeek, Alibaba Cloud Bailian (Qwen), Moonshot (Kimi), Volcengine (Doubao), OpenAI, Google Gemini, Zhipu (GLM), MiniMax, and OpenRouter.
  - Ships with an opt-in local HTTP / WebSocket API bound to `127.0.0.1` and a zero-dependency Node.js MCP bridge ([mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)) for Claude Desktop or Cursor.

## Downloads & Platform Support

Current version: `v0.1.0` (prebuilt binaries are not yet published on [GitHub Releases](https://github.com/hbdzh/ai-terrarium/releases); please build from source).

| Platform | Status | Notes |
| --- | --- | --- |
| Windows | Build from source | Includes custom window chrome integration |
| macOS | Build from source | Native overlay titlebar layout |
| Android | Build from source | Tauri v2 mobile build |
| HarmonyOS / OpenHarmony | Build from source | Adaptation completed (`ohos_compat.rs` and `feat/open-harmony` branch patches) |

<a id="getting-started"></a>

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/) toolchain & Cargo
- [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)

### Build & Run

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

- [Local HTTP / WebSocket API & MCP Bridge](mcp/README.md) (Chinese)

## License

No `LICENSE` file is currently included in this repository. Please check with the maintainer before redistributing.
