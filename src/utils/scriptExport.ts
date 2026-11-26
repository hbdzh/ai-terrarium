import type { ExperimentDetail, ObserverNote, TimelineMessage } from "../types";

export interface PureScriptItem {
  kind: "ai" | "god_system" | "god_public" | "pass" | "observer_note" | "room_header";
  speaker?: string;
  roomLabel?: string;
  content: string;
  colorHex?: string;
  meta?: string;
}

export interface PureScriptDocument {
  title: string;
  tags: string[];
  cast: { label: string; colorHex: string }[];
  boardContent: string;
  items: PureScriptItem[];
}

const ROLE_COLORS = [
  "#334155", // slate
  "#B45309", // amber
  "#047857", // emerald
  "#6D28D9", // violet
  "#0369A1", // sky
  "#BE123C", // rose
];

export function stripThinkTagsClient(raw: string): string {
  if (!raw) return "";
  let out = raw.replace(/<think>[\s\S]*?<\/think>/gi, "");
  const openIdx = out.search(/<think>/i);
  if (openIdx >= 0) {
    out = out.slice(0, openIdx);
  }
  return out.trim();
}

function isPassNoticeClient(msg: TimelineMessage): boolean {
  return msg.role === "GodSystem" && msg.content.endsWith("看了一眼消息，没接话");
}

export function buildPureScriptDocument(detail: ExperimentDetail): PureScriptDocument {
  const agents = detail.config.agents ?? [];
  const agentColorMap = new Map<string, string>();
  const agentLabelMap = new Map<string, string>();

  const cast = agents.map((ag, idx) => {
    const colorHex = ROLE_COLORS[idx % ROLE_COLORS.length];
    agentColorMap.set(ag.id, colorHex);
    agentLabelMap.set(ag.id, ag.label || `智能体 ${idx + 1}`);
    return { label: ag.label || `智能体 ${idx + 1}`, colorHex };
  });

  const labelFor = (id: string | null) =>
    id ? agentLabelMap.get(id) ?? id : "AI";
  const colorFor = (id: string | null) =>
    id ? agentColorMap.get(id) ?? "#334155" : "#334155";

  const notesSorted: ObserverNote[] = [...(detail.observerNotes ?? [])].sort(
    (a, b) => a.triggerTurn - b.triggerTurn
  );
  let noteIdx = 0;
  let lastRoom = "main";
  const items: PureScriptItem[] = [];

  for (const msg of detail.messages) {
    const clean = stripThinkTagsClient(msg.content);
    if (msg.role === "Ai") {
      const roomId = msg.roomId || "main";
      if (roomId !== "main" && roomId !== lastRoom) {
        const members = detail.roomMembers?.[roomId] ?? [];
        const desc =
          members.length > 0
            ? members.map((id) => labelFor(id)).join(" × ")
            : "里屋密谈";
        items.push({
          kind: "room_header",
          content: `🔒 私密里屋 · ${desc}`,
        });
      }
      lastRoom = roomId;

      items.push({
        kind: "ai",
        speaker: labelFor(msg.agentId),
        roomLabel: roomId !== "main" ? "里屋" : undefined,
        content: clean,
        colorHex: colorFor(msg.agentId),
      });
    } else if (msg.role === "GodSystem") {
      if (isPassNoticeClient(msg)) {
        items.push({ kind: "pass", content: clean });
      } else {
        items.push({ kind: "god_system", speaker: "环境旁白", content: clean });
      }
    } else if (msg.role === "GodPublic") {
      items.push({ kind: "god_public", speaker: "路人", content: clean });
    }
    // GodWhisper intentionally excluded from pure script mode

    if (msg.role === "Ai") {
      while (
        noteIdx < notesSorted.length &&
        notesSorted[noteIdx].triggerTurn <= msg.turnNumber
      ) {
        const n = notesSorted[noteIdx];
        const act = n.narrativeAct?.trim() ? `${n.narrativeAct.trim()} · ` : "";
        items.push({
          kind: "observer_note",
          meta: `第 ${n.triggerTurn} 轮 · ${act}${n.depthStage}`,
          content: n.note,
        });
        noteIdx += 1;
      }
    }
  }

  while (noteIdx < notesSorted.length) {
    const n = notesSorted[noteIdx];
    items.push({
      kind: "observer_note",
      meta: `第 ${n.triggerTurn} 轮 · ${n.depthStage}`,
      content: n.note,
    });
    noteIdx += 1;
  }

  return {
    title: detail.title,
    tags: detail.config.tags ?? [],
    cast,
    boardContent: detail.boardContent?.trim() ?? "",
    items,
  };
}

function wrapTextLines(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number
): string[] {
  const paragraphs = text.split(/\r?\n/);
  const lines: string[] = [];
  for (const para of paragraphs) {
    if (!para) {
      lines.push("");
      continue;
    }
    let line = "";
    for (const ch of Array.from(para)) {
      const test = line + ch;
      if (ctx.measureText(test).width > maxWidth && line.length > 0) {
        lines.push(line);
        line = ch;
      } else {
        line = test;
      }
    }
    if (line) lines.push(line);
  }
  return lines;
}

function drawRoundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number
) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/**
 * Renders an Apple book-scroll style high-resolution PNG of the pure script
 * using HTML5 Canvas 2D (zero external network/SVG-taint dependencies).
 */
export async function renderPureScriptPngBytes(
  detail: ExperimentDetail
): Promise<Uint8Array> {
  const doc = buildPureScriptDocument(detail);
  const logicalW = 780;
  const padX = 52;
  const contentW = logicalW - padX * 2;
  const dpr = 2;
  const fontFamily =
    '-apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC", "Microsoft YaHei", sans-serif';

  // Measure pass using a scratch canvas
  const measureCanvas = document.createElement("canvas");
  const mctx = measureCanvas.getContext("2d");
  if (!mctx) throw new Error("无法创建 Canvas 2D 上下文");

  type DrawOp = (ctx: CanvasRenderingContext2D) => void;
  const ops: DrawOp[] = [];
  let y = 56;

  // Title
  mctx.font = `700 26px ${fontFamily}`;
  const titleLines = wrapTextLines(mctx, doc.title, contentW);
  for (const line of titleLines) {
    const drawY = y;
    ops.push((ctx) => {
      ctx.font = `700 26px ${fontFamily}`;
      ctx.fillStyle = "#1D1D1F";
      ctx.fillText(line, padX, drawY + 24);
    });
    y += 34;
  }
  y += 6;

  // Tags
  if (doc.tags.length > 0) {
    let tx = padX;
    const tagRowY = y;
    mctx.font = `500 12px ${fontFamily}`;
    for (const tag of doc.tags) {
      const tw = mctx.measureText(tag).width + 20;
      if (tx + tw > logicalW - padX) {
        tx = padX;
        y += 28;
      }
      const curX = tx;
      const curY = y;
      ops.push((ctx) => {
        drawRoundedRect(ctx, curX, curY, tw, 22, 11);
        ctx.fillStyle = "#EFEFF4";
        ctx.fill();
        ctx.font = `500 12px ${fontFamily}`;
        ctx.fillStyle = "#3A3A3C";
        ctx.fillText(tag, curX + 10, curY + 15);
      });
      tx += tw + 8;
    }
    if (y === tagRowY) y += 26;
    else y += 28;
    y += 6;
  }

  // Cast row
  if (doc.cast.length > 0) {
    let cx = padX;
    mctx.font = `600 13px ${fontFamily}`;
    for (const actor of doc.cast) {
      const aw = mctx.measureText(actor.label).width + 26;
      if (cx + aw > logicalW - padX) {
        cx = padX;
        y += 24;
      }
      const curX = cx;
      const curY = y;
      ops.push((ctx) => {
        ctx.beginPath();
        ctx.arc(curX + 5, curY + 10, 4.5, 0, Math.PI * 2);
        ctx.fillStyle = actor.colorHex;
        ctx.fill();
        ctx.font = `600 13px ${fontFamily}`;
        ctx.fillStyle = "#475569";
        ctx.fillText(actor.label, curX + 14, curY + 14);
      });
      cx += aw + 6;
    }
    y += 30;
  }

  // Divider
  {
    const divY = y;
    ops.push((ctx) => {
      ctx.strokeStyle = "rgba(0,0,0,0.08)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(padX, divY);
      ctx.lineTo(logicalW - padX, divY);
      ctx.stroke();
    });
    y += 26;
  }

  // Shared board summary if non-empty
  if (doc.boardContent) {
    mctx.font = `400 13px ${fontFamily}`;
    const bLines = wrapTextLines(mctx, doc.boardContent, contentW - 32).slice(0, 14);
    const boxH = 38 + bLines.length * 20 + 12;
    const boxY = y;
    ops.push((ctx) => {
      drawRoundedRect(ctx, padX, boxY, contentW, boxH, 14);
      ctx.fillStyle = "#F4F4F6";
      ctx.fill();
      ctx.font = `600 12.5px ${fontFamily}`;
      ctx.fillStyle = "#1D1D1F";
      ctx.fillText("现场线索与共识", padX + 16, boxY + 24);
      ctx.font = `400 13px ${fontFamily}`;
      ctx.fillStyle = "#334155";
      bLines.forEach((ln, idx) => {
        ctx.fillText(ln, padX + 16, boxY + 46 + idx * 20);
      });
    });
    y += boxH + 22;
  }

  // Items
  for (const item of doc.items) {
    if (item.kind === "room_header") {
      const boxY = y;
      ops.push((ctx) => {
        drawRoundedRect(ctx, padX, boxY, contentW, 30, 10);
        ctx.fillStyle = "#F3F0FA";
        ctx.fill();
        ctx.strokeStyle = "#DDD6FE";
        ctx.lineWidth = 1;
        ctx.stroke();
        ctx.font = `600 12px ${fontFamily}`;
        ctx.fillStyle = "#6D28D9";
        ctx.fillText(item.content, padX + 14, boxY + 19);
      });
      y += 42;
      continue;
    }

    if (item.kind === "observer_note") {
      mctx.font = `400 13px ${fontFamily}`;
      const nLines = wrapTextLines(mctx, item.content, contentW - 36);
      const boxH = 34 + nLines.length * 20 + 12;
      const boxY = y;
      ops.push((ctx) => {
        drawRoundedRect(ctx, padX, boxY, contentW, boxH, 12);
        ctx.fillStyle = "#F5F7FA";
        ctx.fill();
        ctx.strokeStyle = "rgba(0,122,255,0.16)";
        ctx.lineWidth = 1;
        ctx.stroke();
        ctx.font = `600 12px ${fontFamily}`;
        ctx.fillStyle = "#0062CC";
        ctx.fillText(`🔭 观察员阶段手记 · ${item.meta ?? ""}`, padX + 16, boxY + 22);
        ctx.font = `400 13px ${fontFamily}`;
        ctx.fillStyle = "#334155";
        nLines.forEach((ln, idx) => {
          ctx.fillText(ln, padX + 16, boxY + 44 + idx * 20);
        });
      });
      y += boxH + 18;
      continue;
    }

    if (item.kind === "god_system" || item.kind === "god_public") {
      const isSys = item.kind === "god_system";
      mctx.font = `400 13.5px ${fontFamily}`;
      const gLines = wrapTextLines(mctx, item.content, contentW - 36);
      const boxH = 32 + gLines.length * 21 + 12;
      const boxY = y;
      ops.push((ctx) => {
        drawRoundedRect(ctx, padX, boxY, contentW, boxH, 12);
        ctx.fillStyle = isSys ? "#1E293B" : "#ECFDF5";
        ctx.fill();
        ctx.font = `600 11.5px ${fontFamily}`;
        ctx.fillStyle = isSys ? "#CBD5E1" : "#047857";
        ctx.fillText(isSys ? "🌐 环境旁白" : "👤 路人插话", padX + 16, boxY + 21);
        ctx.font = `400 13.5px ${fontFamily}`;
        ctx.fillStyle = isSys ? "#F8FAFC" : "#065F46";
        gLines.forEach((ln, idx) => {
          ctx.fillText(ln, padX + 16, boxY + 42 + idx * 21);
        });
      });
      y += boxH + 16;
      continue;
    }

    if (item.kind === "pass") {
      const passY = y;
      ops.push((ctx) => {
        ctx.font = `italic 12px ${fontFamily}`;
        ctx.fillStyle = "#94A3B8";
        ctx.fillText(`— ${item.content} —`, padX + 14, passY + 14);
      });
      y += 26;
      continue;
    }

    // AI dialogue turn
    mctx.font = `400 15px ${fontFamily}`;
    const bodyLines = wrapTextLines(mctx, item.content, contentW - 22);
    const turnY = y;
    const isPrivate = Boolean(item.roomLabel);
    const blockH = 26 + bodyLines.length * 24 + 8;

    ops.push((ctx) => {
      if (isPrivate) {
        drawRoundedRect(ctx, padX, turnY - 4, contentW, blockH + 4, 12);
        ctx.fillStyle = "#FAF8FF";
        ctx.fill();
      }
      ctx.beginPath();
      ctx.arc(padX + 6, turnY + 11, 4.5, 0, Math.PI * 2);
      ctx.fillStyle = item.colorHex ?? "#334155";
      ctx.fill();

      ctx.font = `600 14px ${fontFamily}`;
      ctx.fillStyle = item.colorHex ?? "#1D1D1F";
      const headerText = item.roomLabel
        ? `${item.speaker ?? "AI"}（${item.roomLabel}）`
        : item.speaker ?? "AI";
      ctx.fillText(headerText, padX + 18, turnY + 15);

      ctx.font = `400 15px ${fontFamily}`;
      ctx.fillStyle = "#1D1D1F";
      bodyLines.forEach((ln, idx) => {
        ctx.fillText(ln, padX + 18, turnY + 40 + idx * 24);
      });
    });
    y += blockH + 10;
  }

  // Footer
  y += 12;
  const footY = y;
  ops.push((ctx) => {
    ctx.strokeStyle = "rgba(0,0,0,0.06)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(padX, footY);
    ctx.lineTo(logicalW - padX, footY);
    ctx.stroke();
    ctx.font = `400 11.5px ${fontFamily}`;
    ctx.fillStyle = "#94A3B8";
    ctx.fillText("AI Terrarium · 纯享剧本模式导出", padX, footY + 24);
  });
  y += 54;

  // Guard maximum browser canvas pixel height (16384px limit on WKWebView/GPU backends)
  const effectiveDpr = y * dpr <= 15600 ? dpr : 1;
  const clampedH = Math.min(y, 15600);
  const canvas = document.createElement("canvas");
  canvas.width = logicalW * effectiveDpr;
  canvas.height = clampedH * effectiveDpr;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("无法初始化 Canvas 绘图");

  ctx.scale(effectiveDpr, effectiveDpr);
  // Warm Apple book-paper backdrop
  ctx.fillStyle = "#FCFBF9";
  ctx.fillRect(0, 0, logicalW, clampedH);

  for (const op of ops) {
    op(ctx);
  }

  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((b) => {
      if (b) resolve(b);
      else reject(new Error("PNG 编码失败"));
    }, "image/png");
  });
  const buf = await blob.arrayBuffer();
  return new Uint8Array(buf);
}
