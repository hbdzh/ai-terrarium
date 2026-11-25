<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { Channel, invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import TopControlBar from "./components/TopControlBar.vue";
import ThoughtStream, { type StreamingDraft } from "./components/ThoughtStream.vue";
import ObserverPanel from "./components/ObserverPanel.vue";
import CreatorConsole from "./components/CreatorConsole.vue";
import ReplayController from "./components/ReplayController.vue";
import SharedBoardDrawer from "./components/SharedBoardDrawer.vue";
import SettingsDrawer from "./components/SettingsDrawer.vue";
import NewExperimentModal from "./components/NewExperimentModal.vue";
import type {
  BoardApprovalMode,
  BoardApprovalRequest,
  EngineState,
  ExperimentConfig,
  ExperimentDetail,
  ExperimentSummary,
  PresetTemplate,
  ScenarioPackage,
  SpeakerRole,
  StreamEvent,
  TimelineMessage,
  TurnSnapshot,
} from "./types";

const experiment = ref<ExperimentDetail | null>(null);
const experiments = ref<ExperimentSummary[]>([]);
const presets = ref<PresetTemplate[]>([]);
const engineState = ref<EngineState>("idle");
const statusReason = ref<string | null>(null);
const streaming = ref<StreamingDraft | null>(null);
const observerRunning = ref(false);
const generatingReport = ref(false);
const queueLen = ref(0);
const toastMessage = ref<string | null>(null);

const settingsOpen = ref(false);
const newExpOpen = ref(false);
const boardOpen = ref(false);
const pendingApproval = ref<BoardApprovalRequest | null>(null);
const boardLastActor = ref<string>("");
const boardLastReason = ref<string>("");
const mobileTab = ref<"stream" | "board" | "observer">("stream");
const streamRef = ref<InstanceType<typeof ThoughtStream> | null>(null);

// Theater Replay State
const replayActive = ref(false);
const replayPlaying = ref(false);
const replaySpeed = ref<1 | 2 | 5>(1);
const replayMsgIndex = ref(0);
const replaySnapshots = ref<TurnSnapshot[]>([]);
const replayStreaming = ref<StreamingDraft | null>(null);
let replayTimer: number | null = null;

const canBranch = computed(
  () =>
    !replayActive.value &&
    engineState.value !== "running" &&
    engineState.value !== "stepping"
);

const displayMessages = computed<TimelineMessage[]>(() => {
  if (!experiment.value) return [];
  if (!replayActive.value) return experiment.value.messages;
  return experiment.value.messages.slice(0, replayMsgIndex.value);
});

const replayCurrentTurn = computed(() => {
  if (!replayActive.value || !experiment.value) return experiment.value?.totalTurns ?? 0;
  if (replayStreaming.value) return replayStreaming.value.turnNumber;
  const vis = displayMessages.value;
  return vis.length > 0 ? vis[vis.length - 1].turnNumber : 0;
});

const displayObserverNotes = computed(() => {
  const notes = experiment.value?.observerNotes ?? [];
  if (!replayActive.value) return notes;
  return notes.filter((n) => n.triggerTurn <= replayCurrentTurn.value);
});

const latestCrisisCards = computed(() => {
  const notes = experiment.value?.observerNotes ?? [];
  for (const n of notes) {
    if (n.crisisCards && n.crisisCards.length > 0) {
      return n.crisisCards;
    }
  }
  return [];
});

const displayStreaming = computed(() =>
  replayActive.value ? replayStreaming.value : streaming.value
);

const displayBoardContent = computed(() => {
  if (!experiment.value) return "";
  if (!replayActive.value) return experiment.value.boardContent ?? "";
  const turn = replayCurrentTurn.value;
  const matching = [...replaySnapshots.value]
    .reverse()
    .find((s) => s.turnNumber <= turn);
  return matching ? matching.boardContent : (experiment.value.config.initialBoard ?? "");
});

const displayBoardPrevContent = computed(() => {
  if (!experiment.value) return "";
  if (!replayActive.value) return experiment.value.boardPrevContent ?? "";
  const turn = replayCurrentTurn.value;
  const prevSnap = [...replaySnapshots.value]
    .reverse()
    .find((s) => s.turnNumber < turn);
  return prevSnap ? prevSnap.boardContent : (experiment.value.config.initialBoard ?? "");
});

function stopReplayTimer() {
  if (replayTimer !== null) {
    window.clearTimeout(replayTimer);
    replayTimer = null;
  }
}

function exitReplay() {
  stopReplayTimer();
  replayActive.value = false;
  replayPlaying.value = false;
  replayStreaming.value = null;
  replayMsgIndex.value = 0;
}

async function toggleReplay() {
  if (replayActive.value) {
    exitReplay();
    return;
  }
  if (!experiment.value || experiment.value.messages.length === 0) return;
  try {
    replaySnapshots.value = await invoke<TurnSnapshot[]>("get_experiment_snapshots", {
      experimentId: experiment.value.id,
    });
  } catch {
    replaySnapshots.value = [];
  }
  replayActive.value = true;
  replayMsgIndex.value = 0;
  replayStreaming.value = null;
  replayPlaying.value = true;
  scheduleNextReplayTick();
}

function scheduleNextReplayTick() {
  stopReplayTimer();
  if (!replayActive.value || !replayPlaying.value || !experiment.value) return;

  const allMsgs = experiment.value.messages;
  if (replayMsgIndex.value >= allMsgs.length && !replayStreaming.value) {
    replayPlaying.value = false;
    return;
  }

  const speed = replaySpeed.value;
  if (!replayStreaming.value) {
    const nextMsg = allMsgs[replayMsgIndex.value];
    if (nextMsg.role !== "Ai") {
      replayMsgIndex.value += 1;
      replayTimer = window.setTimeout(scheduleNextReplayTick, Math.round(450 / speed));
      return;
    }
    const label =
      experiment.value.config.agents.find((a) => a.id === nextMsg.agentId)?.label ?? "AI";
    replayStreaming.value = {
      messageId: nextMsg.id,
      agentId: nextMsg.agentId ?? "AiA",
      agentLabel: label,
      turnNumber: nextMsg.turnNumber,
      modelName: nextMsg.modelName,
      reasoningContent: nextMsg.reasoningContent ?? "",
      reasoningStartedAt: null,
      reasoningElapsedMs: 0,
      content: "",
    };
  }

  const targetMsg = allMsgs[replayMsgIndex.value];
  const currentDraft = replayStreaming.value;
  if (!targetMsg || !currentDraft) return;

  const fullText = targetMsg.content ?? "";
  const curLen = currentDraft.content.length;
  if (curLen < fullText.length) {
    const stepChars = Math.max(2, Math.round(4 * speed));
    currentDraft.content = fullText.slice(0, curLen + stepChars);
    replayTimer = window.setTimeout(scheduleNextReplayTick, 28);
  } else {
    replayStreaming.value = null;
    replayMsgIndex.value += 1;
    replayTimer = window.setTimeout(scheduleNextReplayTick, Math.round(500 / speed));
  }
}

function toggleReplayPlay() {
  if (!experiment.value) return;
  if (!replayPlaying.value && replayMsgIndex.value >= experiment.value.messages.length) {
    replayMsgIndex.value = 0;
    replayStreaming.value = null;
  }
  replayPlaying.value = !replayPlaying.value;
  if (replayPlaying.value) {
    scheduleNextReplayTick();
  } else {
    stopReplayTimer();
  }
}

function setReplaySpeed(sp: 1 | 2 | 5) {
  replaySpeed.value = sp;
}

function seekReplay(idx: number) {
  stopReplayTimer();
  replayStreaming.value = null;
  replayMsgIndex.value = idx;
  if (replayPlaying.value) {
    scheduleNextReplayTick();
  }
}

function stepReplay(delta: number) {
  if (!experiment.value) return;
  stopReplayTimer();
  replayPlaying.value = false;
  replayStreaming.value = null;
  const max = experiment.value.messages.length;
  replayMsgIndex.value = Math.max(0, Math.min(max, replayMsgIndex.value + delta));
}

onUnmounted(() => {
  stopReplayTimer();
  const win = window as unknown as { __handleAndroidBack?: () => boolean };
  delete win.__handleAndroidBack;
});

// Touch Swipe Gestures between mobile tabs (< 768px)
let touchStartX = 0;
let touchStartY = 0;
let touchEligible = false;

function onMainTouchStart(e: TouchEvent) {
  if (window.innerWidth >= 768 || e.touches.length !== 1) {
    touchEligible = false;
    return;
  }
  const target = e.target as HTMLElement | null;
  if (target?.closest("pre, code, input, textarea, select, svg, .no-swipe")) {
    touchEligible = false;
    return;
  }
  touchEligible = true;
  touchStartX = e.touches[0].clientX;
  touchStartY = e.touches[0].clientY;
}

function onMainTouchEnd(e: TouchEvent) {
  if (!touchEligible || e.changedTouches.length !== 1) return;
  touchEligible = false;
  const dx = e.changedTouches[0].clientX - touchStartX;
  const dy = e.changedTouches[0].clientY - touchStartY;
  if (Math.abs(dx) < 64 || Math.abs(dx) < Math.abs(dy) * 1.6) return;

  const order: ("stream" | "board" | "observer")[] = ["stream", "board", "observer"];
  const idx = order.indexOf(mobileTab.value);
  if (dx < 0 && idx < order.length - 1) {
    mobileTab.value = order[idx + 1];
  } else if (dx > 0 && idx > 0) {
    mobileTab.value = order[idx - 1];
  }
}

const missingKeys = computed(() => {
  const agents = experiment.value?.config.agents ?? [];
  return agents.some((a) => !a.apiKey.trim());
});

const hasObserverKey = computed(() => {
  const cfg = experiment.value?.config;
  return Boolean(cfg?.nodeC.apiKey.trim() && cfg?.nodeC.modelName.trim());
});

let toastTimer: number | null = null;
function showToast(msg: string) {
  toastMessage.value = msg;
  if (toastTimer) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastMessage.value = null;
  }, 4200);
}

async function refreshExperiments() {
  experiments.value = await invoke<ExperimentSummary[]>("list_experiments");
}

function handleStreamEvent(ev: StreamEvent) {
  if (!experiment.value || ev.experimentId !== experiment.value.id) {
    return;
  }

  switch (ev.type) {
    case "state_changed":
      engineState.value = ev.state;
      statusReason.value = ev.reason;
      if (ev.state === "paused" || ev.state === "idle") {
        streaming.value = null;
        pendingApproval.value = null;
      }
      if (ev.reason) {
        showToast(ev.reason);
      }
      break;

    case "message_added":
      experiment.value.messages.push(ev.message);
      refreshExperiments();
      break;

    case "stream_start":
      streaming.value = {
        messageId: ev.messageId,
        agentId: ev.agentId,
        agentLabel: ev.agentLabel,
        turnNumber: ev.turnNumber,
        modelName: ev.modelName,
        reasoningContent: "",
        reasoningStartedAt: null,
        reasoningElapsedMs: 0,
        content: "",
      };
      break;

    case "stream_reasoning_delta":
      if (streaming.value && streaming.value.messageId === ev.messageId) {
        const now = performance.now();
        if (streaming.value.reasoningStartedAt === null) {
          streaming.value.reasoningStartedAt = now;
        }
        streaming.value.reasoningContent += ev.delta;
        streaming.value.reasoningElapsedMs = Math.max(
          0,
          now - streaming.value.reasoningStartedAt
        );
      }
      break;

    case "stream_delta":
      if (streaming.value && streaming.value.messageId === ev.messageId) {
        if (
          streaming.value.reasoningStartedAt !== null &&
          streaming.value.content.length === 0
        ) {
          streaming.value.reasoningElapsedMs = Math.max(
            streaming.value.reasoningElapsedMs,
            performance.now() - streaming.value.reasoningStartedAt
          );
        }
        streaming.value.content += ev.delta;
      }
      break;

    case "stream_end":
      streaming.value = null;
      experiment.value.messages.push(ev.message);
      experiment.value.totalTurns = ev.totalTurns;
      experiment.value.totalTokens = ev.totalTokens;
      refreshExperiments();
      break;

    case "observer_started":
      observerRunning.value = true;
      break;

    case "observer_completed":
      observerRunning.value = false;
      experiment.value.observerNotes.unshift(ev.note);
      if (ev.note.perturbationInjected) {
        showToast("观察员检测到对话僵化 > 80，已自动注入破局扰动");
      }
      break;

    case "observer_error":
      observerRunning.value = false;
      showToast(`观察员异常: ${ev.error}`);
      break;

    case "observer_skipped":
      observerRunning.value = false;
      break;

    case "memory_compacted":
      experiment.value.compactedUntilSeq = ev.compactedUntilSeq;
      experiment.value.compactedSummary = ev.compactedSummary;
      showToast(
        `上下文已自动折叠为历史纪要（~${ev.tokensBefore.toLocaleString()} → ${ev.tokensAfter.toLocaleString()} tok）`
      );
      break;

    case "tool_call_started":
      showToast(`${ev.agentLabel} 正在调用工具: ${ev.toolName}`);
      break;

    case "board_updated":
      experiment.value.boardContent = ev.boardContent;
      experiment.value.boardPrevContent = ev.boardPrevContent;
      boardLastActor.value = ev.actorLabel;
      boardLastReason.value = ev.reason;
      break;

    case "board_approval_requested":
      pendingApproval.value = {
        experimentId: ev.experimentId,
        requestId: ev.requestId,
        agentId: ev.agentId,
        agentLabel: ev.agentLabel,
        currentContent: ev.currentContent,
        proposedContent: ev.proposedContent,
        reason: ev.reason,
      };
      break;

    case "board_approval_resolved":
      if (pendingApproval.value?.requestId === ev.requestId) {
        pendingApproval.value = null;
      }
      break;

    case "intervention_queued":
      queueLen.value = ev.queueLen;
      break;

    case "engine_error":
      streaming.value = null;
      showToast(ev.message);
      break;
  }
}

onMounted(async () => {
  const win = window as unknown as { __handleAndroidBack?: () => boolean };
  win.__handleAndroidBack = () => {
    if (settingsOpen.value) {
      settingsOpen.value = false;
      return true;
    }
    if (newExpOpen.value) {
      newExpOpen.value = false;
      return true;
    }
    if (mobileTab.value !== "stream") {
      mobileTab.value = "stream";
      return true;
    }
    if (boardOpen.value) {
      boardOpen.value = false;
      return true;
    }
    if (replayActive.value) {
      exitReplay();
      return true;
    }
    return false;
  };

  const ch = new Channel<StreamEvent>();
  ch.onmessage = handleStreamEvent;
  await invoke("register_stream_channel", { channel: ch });

  const [loadedPresets, activeExp, list, state] = await Promise.all([
    invoke<PresetTemplate[]>("get_presets"),
    invoke<ExperimentDetail>("get_active_experiment"),
    invoke<ExperimentSummary[]>("list_experiments"),
    invoke<EngineState>("get_engine_state"),
  ]);
  presets.value = loadedPresets;
  experiment.value = activeExp;
  experiments.value = list;
  engineState.value = state;
});

async function startEngine() {
  if (missingKeys.value) {
    settingsOpen.value = true;
    showToast("请先为所有参与对话的 AI 节点配置 API Key");
    return;
  }
  await invoke("start_engine");
}

async function pauseEngine() {
  await invoke("pause_engine");
}

async function stepEngine() {
  if (missingKeys.value) {
    settingsOpen.value = true;
    showToast("请先为所有参与对话的 AI 节点配置 API Key");
    return;
  }
  await invoke("step_engine");
}

async function saveConfig(newConfig: ExperimentConfig) {
  if (!experiment.value) return;
  const updated = await invoke<ExperimentDetail>("update_experiment", {
    experimentId: experiment.value.id,
    title: experiment.value.title,
    config: newConfig,
  });
  experiment.value = updated;
  await refreshExperiments();
  showToast("配置已保存");
}

async function handleSavePreset(preset: PresetTemplate) {
  presets.value = await invoke<PresetTemplate[]>("save_preset", { preset });
  showToast(`预设模板「${preset.name}」已保存`);
}

async function handleResetPreset(presetId: string) {
  presets.value = await invoke<PresetTemplate[]>("reset_builtin_preset", { presetId });
  showToast("已恢复预设默认提示词");
}

async function handleDeletePreset(presetId: string) {
  presets.value = await invoke<PresetTemplate[]>("delete_preset", { presetId });
  showToast("自定义预设已删除");
}

async function renameExperiment(newTitle: string) {
  if (!experiment.value) return;
  const updated = await invoke<ExperimentDetail>("update_experiment", {
    experimentId: experiment.value.id,
    title: newTitle,
    config: experiment.value.config,
  });
  experiment.value = updated;
  await refreshExperiments();
}

async function selectExperiment(id: string) {
  exitReplay();
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  boardLastActor.value = "";
  boardLastReason.value = "";
  experiment.value = await invoke<ExperimentDetail>("switch_experiment", {
    experimentId: id,
  });
  await refreshExperiments();
}

async function createNewExperiment(title: string, config: ExperimentConfig) {
  exitReplay();
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  boardLastActor.value = "";
  boardLastReason.value = "";
  experiment.value = await invoke<ExperimentDetail>("create_experiment", {
    title,
    config,
  });
  await refreshExperiments();
}

async function handleFork(targetTurn: number) {
  if (!experiment.value) return;
  exitReplay();
  streaming.value = null;
  queueLen.value = 0;
  const forked = await invoke<ExperimentDetail>("fork_experiment", {
    sourceExpId: experiment.value.id,
    targetTurn,
    editMessageId: null,
    editContent: null,
  });
  experiment.value = forked;
  await refreshExperiments();
  showToast(`已从第 ${targetTurn} 轮开启平行宇宙分支「${forked.title}」`);
}

async function handleEditAndReplay(payload: {
  messageId: string;
  turnNumber: number;
  content: string;
}) {
  if (!experiment.value) return;
  exitReplay();
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  const forked = await invoke<ExperimentDetail>("fork_experiment", {
    sourceExpId: experiment.value.id,
    targetTurn: payload.turnNumber,
    editMessageId: payload.messageId,
    editContent: payload.content,
  });
  experiment.value = forked;
  await refreshExperiments();
  showToast(`已改写第 ${payload.turnNumber} 轮并创建重演分支`);
  if (!missingKeys.value) {
    await invoke("step_engine");
  }
}

async function handleSaveBoard(content: string) {
  if (!experiment.value) return;
  const [prev, saved] = await invoke<[string, string]>("update_shared_board", {
    experimentId: experiment.value.id,
    content,
  });
  experiment.value.boardPrevContent = prev;
  experiment.value.boardContent = saved;
  showToast("共享白板已保存");
}

async function handleToggleApprovalMode(mode: BoardApprovalMode) {
  if (!experiment.value) return;
  const updatedConfig = await invoke<ExperimentConfig>("set_board_approval_mode", {
    experimentId: experiment.value.id,
    mode,
  });
  experiment.value.config = updatedConfig;
  showToast(mode === "approval" ? "已开启造物主白板审批拦截" : "已切换为白板自动放行");
}

async function handleResolveApproval(payload: {
  requestId: string;
  approved: boolean;
  modifiedContent?: string;
  rejectReason?: string;
}) {
  await invoke<boolean>("resolve_board_approval", {
    requestId: payload.requestId,
    approved: payload.approved,
    modifiedContent: payload.modifiedContent ?? null,
    rejectReason: payload.rejectReason ?? null,
  });
  pendingApproval.value = null;
}

async function deleteExperiment(id: string) {
  exitReplay();
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  boardLastActor.value = "";
  boardLastReason.value = "";
  const next = await invoke<ExperimentDetail>("delete_experiment", {
    experimentId: id,
  });
  experiment.value = next;
  await refreshExperiments();
}

async function enqueueIntervention(
  kind: SpeakerRole,
  agentId: string | null,
  content: string
) {
  if (!experiment.value) return;
  const len = await invoke<number>("enqueue_intervention", {
    experimentId: experiment.value.id,
    input: { kind, agentId, content },
  });
  queueLen.value = len;
}

async function triggerObserverNow() {
  if (!experiment.value) return;
  await invoke("trigger_observer_now", {
    experimentId: experiment.value.id,
  });
}

async function handleGenerateReport() {
  if (!experiment.value || generatingReport.value) return;
  generatingReport.value = true;
  try {
    const report = await invoke<string>("generate_final_report", {
      experimentId: experiment.value.id,
    });
    experiment.value.finalReport = report;
    showToast("结案复盘简报已生成");
  } catch (err: any) {
    showToast(`生成复盘简报失败: ${err?.toString?.() ?? err}`);
  } finally {
    generatingReport.value = false;
  }
}

async function handleExportScenario() {
  if (!experiment.value) return;
  const pkg = await invoke<ScenarioPackage>("export_scenario_package", {
    experimentId: experiment.value.id,
    includeKeys: false,
  });
  const jsonText = JSON.stringify(pkg, null, 2);
  const safeName = experiment.value.title.replace(/[\\/:*?"<>|]/g, "_");
  try {
    const filePath = await save({
      defaultPath: `${safeName}.scenario.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (filePath) {
      await invoke("save_markdown_to_file", { path: filePath, content: jsonText });
      showToast("剧本模板已导出（已自动脱敏 API Key）");
    }
  } catch {
    try {
      await navigator.clipboard.writeText(jsonText);
      showToast("移动端暂不支持直接保存文件，已将剧本 JSON 复制到剪贴板");
    } catch {
      const blob = new Blob([jsonText], { type: "application/json;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${safeName}.scenario.json`;
      a.click();
      URL.revokeObjectURL(url);
      showToast("已触发剧本 JSON 下载");
    }
  }
}

async function handleImportScenario(file: File) {
  try {
    const text = await file.text();
    const raw = JSON.parse(text);
    const pkg: ScenarioPackage =
      raw && typeof raw === "object" && "config" in raw
        ? {
            version: typeof raw.version === "string" ? raw.version : "2.0",
            title:
              typeof raw.title === "string" && raw.title.trim()
                ? raw.title.trim()
                : file.name.replace(/\.json$/i, ""),
            config: raw.config,
            initialBoard:
              typeof raw.initialBoard === "string"
                ? raw.initialBoard
                : (raw.config?.initialBoard ?? ""),
          }
        : {
            version: "2.0",
            title: file.name.replace(/\.json$/i, ""),
            config: raw,
            initialBoard: raw?.initialBoard ?? "",
          };

    exitReplay();
    streaming.value = null;
    queueLen.value = 0;
    pendingApproval.value = null;
    const created = await invoke<ExperimentDetail>("import_scenario_package", { pkg });
    experiment.value = created;
    await refreshExperiments();
    showToast(`已导入剧本「${created.title}」并就绪开局`);
  } catch (err: any) {
    showToast(`剧本导入失败: ${err?.message ?? err}`);
  }
}

async function exportMarkdown() {
  if (!experiment.value) return;
  const md = await invoke<string>("export_experiment_markdown", {
    experimentId: experiment.value.id,
  });
  const safeName = experiment.value.title.replace(/[\\/:*?"<>|]/g, "_");
  try {
    const filePath = await save({
      defaultPath: `${safeName}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (filePath) {
      await invoke("save_markdown_to_file", { path: filePath, content: md });
      showToast("Markdown 实验档案已导出");
    }
  } catch {
    try {
      await navigator.clipboard.writeText(md);
      showToast("移动端已将完整 Markdown 实验档案复制到剪贴板");
    } catch {
      const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${safeName}.md`;
      a.click();
      URL.revokeObjectURL(url);
      showToast("已通过浏览器下载 Markdown 档案");
    }
  }
}

async function handleJumpToTurn(turnNumber: number) {
  mobileTab.value = "stream";
  await nextTick();
  setTimeout(() => {
    streamRef.value?.scrollToTurn(turnNumber);
  }, 80);
}
</script>

<template>
  <div class="h-dvh w-screen flex flex-col bg-[#F6F6F8] text-[#1D1D1F] overflow-hidden">
    <TopControlBar
      :experiment="experiment"
      :experiments="experiments"
      :engine-state="engineState"
      :status-reason="statusReason"
      :missing-keys="missingKeys"
      :replay-active="replayActive"
      :board-open="boardOpen"
      :has-pending-approval="Boolean(pendingApproval)"
      @start="startEngine"
      @pause="pauseEngine"
      @step="stepEngine"
      @export-md="exportMarkdown"
      @open-settings="settingsOpen = true"
      @open-new-experiment="newExpOpen = true"
      @select-experiment="selectExperiment"
      @delete-experiment="deleteExperiment"
      @rename-experiment="renameExperiment"
      @toggle-replay="toggleReplay"
      @toggle-board="boardOpen = !boardOpen"
      @export-scenario="handleExportScenario"
      @import-scenario="handleImportScenario"
    />

    <!-- Mobile View Switcher (< md) -->
    <div
      class="md:hidden shrink-0 px-3 py-1.5 bg-white/90 border-b border-black/[0.05] flex items-center justify-between gap-2"
    >
      <div class="flex p-0.5 rounded-full bg-[#F2F2F5] border border-black/[0.04]">
        <button
          @click="mobileTab = 'stream'"
          class="px-3 py-1 rounded-full text-[11px] font-medium transition"
          :class="mobileTab === 'stream' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
        >
          对话流 ({{ experiment?.totalTurns ?? 0 }})
        </button>
        <button
          @click="mobileTab = 'board'"
          class="px-3 py-1 rounded-full text-[11px] font-medium transition"
          :class="mobileTab === 'board' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
        >
          白板
        </button>
        <button
          @click="mobileTab = 'observer'"
          class="px-3 py-1 rounded-full text-[11px] font-medium transition"
          :class="mobileTab === 'observer' ? 'bg-[#1D1D1F] text-white' : 'text-slate-600'"
        >
          观察舱 ({{ experiment?.observerNotes.length ?? 0 }})
        </button>
      </div>
      <span class="text-[11px] text-slate-500 tabular-nums">
        {{ experiment?.totalTokens.toLocaleString() ?? 0 }} tok
      </span>
    </div>

    <!-- Main Split Stage: Main Stream (60%) / Collapsible Shared Board / Observer Horizon (40%) -->
    <main
      @touchstart.passive="onMainTouchStart"
      @touchend.passive="onMainTouchEnd"
      class="flex-1 flex min-h-0 overflow-hidden"
    >
      <section
        class="relative flex-1 h-full flex-col bg-white/60 min-w-0"
        :class="mobileTab === 'stream' ? 'flex' : 'hidden md:flex'"
      >
        <ThoughtStream
          ref="streamRef"
          :messages="displayMessages"
          :streaming="displayStreaming"
          :config="experiment?.config ?? null"
          :compacted-summary="experiment?.compactedSummary ?? ''"
          :can-branch="canBranch"
          @fork="handleFork"
          @edit-and-replay="handleEditAndReplay"
        />

        <ReplayController
          v-if="replayActive"
          :playing="replayPlaying"
          :speed="replaySpeed"
          :current-index="replayMsgIndex"
          :total-count="experiment?.messages.length ?? 0"
          :current-turn="replayCurrentTurn"
          :total-turns="experiment?.totalTurns ?? 0"
          @toggle-play="toggleReplayPlay"
          @set-speed="setReplaySpeed"
          @seek="seekReplay"
          @step-prev="stepReplay(-1)"
          @step-next="stepReplay(1)"
          @exit="exitReplay"
        />
        <CreatorConsole
          v-else
          :queue-len="queueLen"
          :agents="experiment?.config.agents ?? []"
          :pending-approval="pendingApproval"
          :crisis-cards="latestCrisisCards"
          :observer-running="observerRunning"
          @submit="enqueueIntervention"
          @resolve-approval="handleResolveApproval"
          @trigger-observer="triggerObserverNow"
        />
      </section>

      <SharedBoardDrawer
        :open="boardOpen || mobileTab === 'board'"
        :class="boardOpen ? (mobileTab === 'board' ? 'flex' : 'hidden md:flex') : 'flex md:hidden'"
        :board-content="displayBoardContent"
        :board-prev-content="displayBoardPrevContent"
        :approval-mode="experiment?.config.boardApprovalMode ?? 'auto'"
        :last-actor="boardLastActor"
        :last-reason="boardLastReason"
        :read-only="replayActive"
        @close="boardOpen = false; mobileTab = 'stream'"
        @save-board="handleSaveBoard"
        @toggle-approval-mode="handleToggleApprovalMode"
      />

      <section
        class="w-full md:w-[38%] lg:w-[36%] xl:w-[40%] max-w-[520px] h-full md:min-w-[290px]"
        :class="mobileTab === 'observer' ? 'block' : 'hidden md:block'"
      >
        <ObserverPanel
          :notes="displayObserverNotes"
          :observer-running="observerRunning"
          :observer-interval="experiment?.config.observerInterval ?? 10"
          :total-turns="replayActive ? replayCurrentTurn : (experiment?.totalTurns ?? 0)"
          :has-observer-key="hasObserverKey"
          :final-report="experiment?.finalReport ?? ''"
          :generating-report="generatingReport"
          @jump-to-turn="handleJumpToTurn"
          @trigger-now="triggerObserverNow"
          @generate-report="handleGenerateReport"
        />
      </section>
    </main>

    <!-- Floating Apple-style Toast -->
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 -translate-y-2"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="toastMessage"
        class="fixed top-16 left-1/2 -translate-x-1/2 z-50 px-4 py-2 rounded-full bg-[#1D1D1F]/90 backdrop-blur-md text-white text-xs font-medium shadow-lg max-w-[90vw] text-center"
      >
        {{ toastMessage }}
      </div>
    </Transition>

    <SettingsDrawer
      :open="settingsOpen"
      :config="experiment?.config ?? null"
      :presets="presets"
      @close="settingsOpen = false"
      @save="saveConfig"
      @save-preset="handleSavePreset"
      @reset-preset="handleResetPreset"
      @delete-preset="handleDeletePreset"
      @notify="showToast"
    />

    <NewExperimentModal
      :open="newExpOpen"
      :presets="presets"
      :current-config="experiment?.config ?? null"
      @close="newExpOpen = false"
      @create="createNewExperiment"
      @save-preset="handleSavePreset"
    />
  </div>
</template>
