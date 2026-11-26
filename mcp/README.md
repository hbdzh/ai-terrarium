# AI Terrarium 本地外部控制接口与 MCP 桥接

AI Terrarium 内置了一个默认关闭、仅监听 `127.0.0.1` 的本地 HTTP + WebSocket 控制接口，并附带零依赖的 Node.js MCP 桥接脚本 [`terrarium_mcp.mjs`](./terrarium_mcp.mjs)。

## 1. 开启本地接口

1. 打开 **AI Terrarium → 设置 → 通用**。
2. 开启 **「外部联动接口（本地 HTTP / WS）」**（默认端口 `17890`，可选配置访问密钥）。
3. 点击 **保存配置** 即可热启动，无需重启应用。

---

## 2. Claude Desktop / Cursor MCP 一键配置

确保本机已安装 Node.js（18+），将下方 JSON 片段复制到 Claude Desktop（`claude_desktop_config.json`）或 Cursor MCP 设置中（替换路径为你的实际仓库路径）：

```json
{
  "mcpServers": {
    "ai-terrarium": {
      "command": "node",
      "args": ["D:/Code/Rust/AIChatStudio/mcp/terrarium_mcp.mjs"],
      "env": {
        "TERRARIUM_API_URL": "http://127.0.0.1:17890",
        "TERRARIUM_API_TOKEN": ""
      }
    }
  }
}
```

> 若在设置中填写了访问密钥，请将密钥填入 `TERRARIUM_API_TOKEN`。

---

## 3. HTTP & WebSocket 接口速查

所有接口仅绑定 `127.0.0.1:{port}`；若设置了访问密钥，需在请求头携带 `Authorization: Bearer <token>` 或在 URL Query 中附带 `?token=<token>`。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/status` | 获取当前推演 ID、标题、状态、轮数、盲猜模式与角色列表 |
| `GET` | `/api/messages?limit=50` | 获取最近 N 条消息（支持 `include_hidden=true` 获取隐藏内容） |
| `GET` | `/api/board` | 获取当前共享白板 Markdown |
| `POST` | `/api/control/start` | 启动自动推演 |
| `POST` | `/api/control/pause` | 暂停推演 |
| `POST` | `/api/control/step` | 推进单步（可选 body: `{"agent_id": "..."}`） |
| `POST` | `/api/intervene/event` | 注入场景事件（body: `{"content": "..."}`） |
| `POST` | `/api/intervene/whisper` | 定向提示（body: `{"agent_id": "...", "content": "..."}`） |
| `POST` | `/api/intervene/speak` | 亲自/弹幕插话（body: `{"sender_name": "观众弹幕", "content": "..."}`） |
| `POST` | `/api/intervene/private_room` | 开启限时私密房（body: `{"agent_ids": ["id1", "id2"], "turns": 3}`） |
| `POST` | `/api/cards/draw` | 抽取 3 张转折事件卡（可选 `{"inject_index": 0}` 直接注入） |
| `GET` | `/ws` | WebSocket 实时推送 `message_appended` / `board_updated` / `observer_report` / `status_changed` |
