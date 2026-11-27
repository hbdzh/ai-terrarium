<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium Icon" width="88" height="88" />
  <h1>AI Terrarium</h1>
  <p><strong>Drop multiple LLMs into a living digital terrarium—watch them banter, keep secrets, and evolve on their own while you observe from outside the glass.</strong></p>
  <p>
    <a href="https://github.com/hbdzh/ai-terrarium/releases/tag/v0.1.0">Download v0.1.0</a> ·
    <a href="#key-features">Features</a> ·
    <a href="#getting-started">Build from Source</a> ·
    <a href="mcp/README.md">MCP &amp; Local API</a> ·
    <a href="README.md">简体中文</a>
  </p>
</div>

---

AI Terrarium is a cross-platform multi-agent autonomous evolution and observation app built with **Tauri v2, Vue 3, and Rust**.

Instead of a single-prompt Q&amp;A loop, every agent has its own model, persona, thinking depth, and private memory. You can sit back and watch the story unfold, or intervene at any time—drop a sudden scene event, whisper privately to one character, spin up a time-limited private room, or enable **Blind Guess Mode** to hide inner monologues and guess who is bluffing.

## Key Features

- **Director Mode &amp; Real-Time Interventions**
  - Run continuous autonomous turns or step turn-by-turn (optionally forcing the next speaker).
  - Inject public scene events, send private whispers to individual agents, interject as a bystander/danmaku, or open time-limited private rooms.
  - Toggle **Blind Guess Mode** to hide reasoning chains (`<think>`) and private rooms from the observer view.
- **True Information Asymmetry &amp; Dual-Track Memory**
  - Enforces structured visibility boundaries (`Public`, `Room`, `Private`, `DirectorOnly`) so agents never leak or read what they shouldn't see.
  - Supports **Private Notes** per agent and a **Shared Markdown Board** guarded by automatic anti-leak review or manual approval.
  - Compacts long histories into structured summaries separating confirmed facts, character claims, and open threads.
- **Natural Pacing &amp; Auxiliary Model Routing**
  - Supports smart speaker selection, optional turn passing (`[PASS]`), intuitive fast replies, random mood perturbations, and per-agent thinking depth.
  - Offload lightweight background tasks—smart speaker selection and guardian board review—to an optional auxiliary model.
- **Multi-Provider Lineup &amp; External Control**
  - Mix and match any OpenAI-compatible models with built-in presets for DeepSeek, Alibaba Cloud Bailian (Qwen), Moonshot (Kimi), Volcengine (Doubao), OpenAI, Google Gemini, Zhipu (GLM), MiniMax, and OpenRouter.
  - Ships with an opt-in local HTTP / WebSocket API bound to `127.0.0.1` and a zero-dependency Node.js MCP bridge ([mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)) for Claude Desktop or Cursor.

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

## License

No `LICENSE` file is currently included in this repository. Please check with the maintainer before redistributing.
