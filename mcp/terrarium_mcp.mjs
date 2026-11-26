#!/usr/bin/env node
/**
 * AI Terrarium - Zero-Dependency MCP (Model Context Protocol) Bridge
 * Connects Claude Desktop / Cursor / any stdio MCP client to the local AI Terrarium HTTP API.
 *
 * Env:
 *   TERRARIUM_API_URL   (default: http://127.0.0.1:17890)
 *   TERRARIUM_API_TOKEN (optional: Bearer token if configured in Settings)
 */

import readline from "node:readline";
import process from "node:process";

const BASE_URL = (process.env.TERRARIUM_API_URL || "http://127.0.0.1:17890").replace(/\/+$/, "");
const API_TOKEN = (process.env.TERRARIUM_API_TOKEN || "").trim();

const TOOLS = [
  {
    name: "get_sandbox_state",
    description: "获取当前 AI Terrarium 推演状态、角色列表与最新对话记录。",
    inputSchema: {
      type: "object",
      properties: {
        limit: {
          type: "integer",
          description: "返回的最新消息条数（默认 20，最大 200）",
          default: 20,
        },
        include_hidden: {
          type: "boolean",
          description: "是否在盲猜模式下也返回思考链与私密房间内容（导演模式设为 true）",
          default: false,
        },
      },
    },
  },
  {
    name: "get_shared_board",
    description: "读取当前实验的共享白板（Markdown 公共记忆板）内容。",
    inputSchema: {
      type: "object",
      properties: {},
    },
  },
  {
    name: "step_turn",
    description: "推进一轮对话，或强制指定下一位发言角色。",
    inputSchema: {
      type: "object",
      properties: {
        agent_id: {
          type: "string",
          description: "可选：指定强制发言的角色 ID（留空则按智能调度自动选择）",
        },
      },
    },
  },
  {
    name: "inject_scene_event",
    description: "向全员注入一条客观突发场景事件（旁白，全员可见）。",
    inputSchema: {
      type: "object",
      properties: {
        content: {
          type: "string",
          description: "客观场景事件描述内容",
        },
      },
      required: ["content"],
    },
  },
  {
    name: "send_private_whisper",
    description: "向单个角色发送定向内心暗示（仅该角色知情，其余角色看不到）。",
    inputSchema: {
      type: "object",
      properties: {
        agent_id: {
          type: "string",
          description: "目标角色 ID",
        },
        content: {
          type: "string",
          description: "注入的内心暗示或潜意识提示内容",
        },
      },
      required: ["agent_id", "content"],
    },
  },
  {
    name: "open_private_room",
    description: "拉取指定多名角色进入限时私密房间（大厅其余角色听不到密谈内容）。",
    inputSchema: {
      type: "object",
      properties: {
        agent_ids: {
          type: "array",
          items: { type: "string" },
          description: "进入私密房间的角色 ID 列表（至少 2 名）",
        },
        turns: {
          type: "integer",
          description: "私密房间持续轮数（1~5 轮，默认 3）",
          default: 3,
        },
      },
      required: ["agent_ids"],
    },
  },
  {
    name: "draw_crisis_cards",
    description: "触发并抽取 3 张实时剧情转折/危机事件卡，可选直接注入其中一张。",
    inputSchema: {
      type: "object",
      properties: {
        inject_index: {
          type: "integer",
          description: "可选：抽取后直接注入第几张卡片（0、1 或 2）",
        },
      },
    },
  },
];

async function callApi(path, method = "GET", body = undefined, timeoutMs = 15000) {
  const headers = { "Content-Type": "application/json" };
  if (API_TOKEN) {
    headers["Authorization"] = `Bearer ${API_TOKEN}`;
  }
  const res = await fetch(`${BASE_URL}${path}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(timeoutMs),
  });
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { ok: res.ok, status: res.status, raw: text };
  }
}

async function handleToolCall(name, args = {}) {
  switch (name) {
    case "get_sandbox_state": {
      const limit = Number.isFinite(args.limit) ? args.limit : 20;
      const includeHidden = Boolean(args.include_hidden);
      const [statusRes, msgsRes] = await Promise.all([
        callApi("/api/status"),
        callApi(`/api/messages?limit=${limit}&include_hidden=${includeHidden}`),
      ]);
      return {
        ok: Boolean(statusRes.ok && msgsRes.ok),
        status: statusRes.data ?? statusRes,
        messages: msgsRes.data ?? msgsRes,
      };
    }
    case "get_shared_board":
      return await callApi("/api/board");
    case "step_turn":
      return await callApi("/api/control/step", "POST", {
        agent_id: args.agent_id || undefined,
      });
    case "inject_scene_event":
      return await callApi("/api/intervene/event", "POST", {
        content: String(args.content || ""),
      });
    case "send_private_whisper":
      return await callApi("/api/intervene/whisper", "POST", {
        agent_id: String(args.agent_id || ""),
        content: String(args.content || ""),
      });
    case "open_private_room":
      return await callApi("/api/intervene/private_room", "POST", {
        agent_ids: Array.isArray(args.agent_ids) ? args.agent_ids : [],
        turns: Number.isFinite(args.turns) ? args.turns : 3,
      });
    case "draw_crisis_cards":
      return await callApi(
        "/api/cards/draw",
        "POST",
        {
          inject_index: Number.isInteger(args.inject_index) ? args.inject_index : undefined,
        },
        65000
      );
    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

function sendResponse(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n");
}

function sendError(id, code, message) {
  process.stdout.write(
    JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } }) + "\n"
  );
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

rl.on("line", async (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;

  let req;
  try {
    req = JSON.parse(trimmed);
  } catch {
    return;
  }

  const { id, method, params } = req;
  // Notifications without id do not expect a response
  if (id === undefined || id === null) {
    return;
  }

  try {
    if (method === "initialize") {
      sendResponse(id, {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "ai-terrarium-mcp", version: "1.0.0" },
      });
    } else if (method === "ping") {
      sendResponse(id, {});
    } else if (method === "tools/list") {
      sendResponse(id, { tools: TOOLS });
    } else if (method === "tools/call") {
      const toolName = params?.name;
      const toolArgs = params?.arguments ?? {};
      const data = await handleToolCall(toolName, toolArgs);
      sendResponse(id, {
        content: [{ type: "text", text: JSON.stringify(data, null, 2) }],
        isError: data?.ok === false,
      });
    } else {
      sendError(id, -32601, `Method not found: ${method}`);
    }
  } catch (err) {
    sendError(id, -32000, err?.message || String(err));
  }
});
