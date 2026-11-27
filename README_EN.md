<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="AI Terrarium Icon" width="88" height="88" />
  <h1>AI Terrarium</h1>
  <p><strong>Put multiple AI characters in one digital terrarium and let them talk on their own—while you play director from outside the glass.</strong></p>
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

Most AI apps are just you asking and the AI answering. **AI Terrarium** flips that around: you set up a scene, pick a few characters powered by different models and personas (DeepSeek, Qwen, Kimi, GPT, Gemini, and more), and let them converse with each other.

You can sit back and watch the drama unfold naturally, or step in at any moment—announce a sudden blackout, slip a secret note to one character, pull two of them into a private room for a few turns, or hide their inner thoughts so you can guess who is bluffing.

<a id="key-features"></a>

## Key Features

- **Play Director in Real Time**
  - **Auto-run or step-by-step**: Let the group chat run continuously, pause anytime, advance one turn at a time, or pick who speaks next.
  - **Four ways to steer the story**:
    - **Scene Events**: Drop an announcement everyone can see (e.g. *"Someone knocks on the door"*).
    - **Private Whispers**: Send a secret message to just one character—nobody else sees it.
    - **Bystander / Danmaku**: Jump in as a passerby and watch the characters react.
    - **Timed Private Rooms**: Pull selected characters into a separate room for a few turns; outsiders never know what was said inside.
  - **Blind Guess Mode**: Hide reasoning chains (`<think>`) and private rooms so you only see public lines—great for mystery, social deduction, or guessing games.
- **Real Secrets, No Accidental Leaks**
  - **Strict room isolation**: Public chat, private rooms, and secret whispers stay strictly separated so characters don't read what they shouldn't know.
  - **Personal notes + shared board**: Characters can jot down private thoughts for themselves, and collaborate on a shared Markdown board—with automatic anti-leak checks or manual approval before edits go live.
  - **Memory that lasts**: Long conversations are summarized into what actually happened, what each character claims, and unresolved mysteries, keeping long sessions coherent.
- **More Human-Like Pacing**
  - **No forced filler**: Characters can pass their turn (`[PASS]`) when they have nothing to add, reply in quick casual bursts, or get subtle random mood shifts.
  - **Offload helper tasks**: Optionally assign a faster, cheaper model to handle background tasks like picking the next speaker and checking the shared board for leaks.

<a id="additional-features"></a>

## Additional Features

- **Mix &amp; Match Models**: Works with any OpenAI-compatible API, with built-in presets for DeepSeek, Alibaba Cloud Bailian (Qwen), Moonshot (Kimi), Volcengine (Doubao), OpenAI, Google Gemini, Zhipu (GLM), MiniMax, and OpenRouter. API profiles are managed separately from character presets.
- **Fork &amp; Compare Parallel Timelines**: Branch off from an experiment mid-way, tweak a rule or persona, and see how differently things turn out.
- **Observer Reports &amp; Twist Cards**: Have a background observer summarize the situation periodically, or draw random twist cards to shake up the scene.
- **Character Library, Tags &amp; Script View**: Includes reusable character presets, style/rule tags, scenario import/export, and a clean script-only reading/print view.
- **Clean Reading Experience**: Collapsible thinking blocks, code highlighting, KaTeX math rendering, and Light / Dark / System themes.
- **Local Storage &amp; MCP Control**: Data stays locally in SQLite; includes an opt-in local HTTP / WebSocket API (`127.0.0.1`) and a zero-dependency Node.js MCP bridge ([mcp/terrarium_mcp.mjs](mcp/terrarium_mcp.mjs)) for Claude Desktop or Cursor.

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
