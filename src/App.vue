<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useWindowSize } from "@vueuse/core";
import { Channel, invoke } from "@tauri-apps/api/core";
import { Telescope, ClipboardList, X, Copy, Check, Code2, Scissors, ClipboardPaste, TextSelect, PanelLeft, PanelRight } from "lucide-vue-next";
import { isMacOS, isWindows, isDesktopOS, isMobileOS } from "./utils/platform";
import { writeClipboardText, readClipboardText } from "./utils/clipboard";
import { warmupMarkdownParser } from "./utils/markdown";
import { triggerHaptic } from "./utils/native";
import { resolvedTheme, setThemePreference, syncThemeVisuals, themePreference } from "./utils/theme";
import { initialDataPromises } from "./bootstrap";
import { seedApiProfilesFromConfig } from "./utils/config";
import { renderPureScriptPngBytes } from "./utils/scriptExport";
import WindowsTitleBar from "./components/WindowsTitleBar.vue";
import ResizablePanelGroup from "./components/ui/ResizablePanelGroup.vue";
import ResizablePanel from "./components/ui/ResizablePanel.vue";
import ResizableHandle from "./components/ui/ResizableHandle.vue";
import TopControlBar from "./components/TopControlBar.vue";
import LeftSidebar from "./components/LeftSidebar.vue";
import ThoughtStream, { type StreamingDraft } from "./components/ThoughtStream.vue";
import ObserverPanel from "./components/ObserverPanel.vue";
import CreatorConsole from "./components/CreatorConsole.vue";
import PureScriptPrintView from "./components/PureScriptPrintView.vue";

const AdaptiveModal = defineAsyncComponent(() => import("./components/ui/AdaptiveModal.vue"));
const Sheet = defineAsyncComponent(() => import("./components/ui/Sheet.vue"));
const ReplayController = defineAsyncComponent(() => import("./components/ReplayController.vue"));
const SharedBoardDrawer = defineAsyncComponent(() => import("./components/SharedBoardDrawer.vue"));
const SettingsDrawer = defineAsyncComponent(() => import("./components/SettingsDrawer.vue"));
const ScenarioScriptModal = defineAsyncComponent(() => import("./components/ScenarioScriptModal.vue"));
const NewExperimentModal = defineAsyncComponent(() => import("./components/NewExperimentModal.vue"));
const ForkTweakModal = defineAsyncComponent(() => import("./components/ForkTweakModal.vue"));
import type {
  ApiProfile,
  BoardApprovalMode,
  BoardApprovalRequest,
  CharacterCard,
  DistillImprintsResponse,
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
  UiPrefs,
} from "./types";

const experiment = ref<ExperimentDetail | null>(null);
const experiments = ref<ExperimentSummary[]>([]);
const presets = ref<PresetTemplate[]>([]);
const characterCards = ref<CharacterCard[]>([]);
const apiProfiles = ref<ApiProfile[]>([]);
const engineState = ref<EngineState>("idle");
const statusReason = ref<string | null>(null);
const streaming = ref<StreamingDraft | null>(null);
const observerRunning = ref(false);
const generatingReport = ref(false);
const distillingImprints = ref(false);
const comparisonReport = ref("");
const generatingComparison = ref(false);
const blindGuessMode = ref(false);
const forkTweakOpen = ref(false);
const directorBusy = ref(false);
const finalePromptVisible = ref(false);
const queueLen = ref(0);
const toastMessage = ref<string | null>(null);
const contextUsagePct = ref<number | null>(null);
const contextActiveTokens = ref<number>(0);
const observerPanelRef = ref<InstanceType<typeof ObserverPanel> | null>(null);

interface RawPromptMessageItem {
  role: string;
  content?: string | null;
}

interface RawPromptPayload {
  model?: string;
  temperature?: number;
  reasoning_effort?: string | null;
  private_note?: string | null;
  messages?: RawPromptMessageItem[];
  tools?: unknown;
}

const promptModalOpen = ref(false);
const promptModalRaw = ref<string | null>(null);
const promptModalParsed = ref<RawPromptPayload | null>(null);
const promptCopied = ref(false);

const settingsOpen = ref(false);
const settingsDrawerRef = ref<{ requestClose?: () => void } | null>(null);
const scriptModalOpen = ref(false);
const scriptModalRef = ref<{ requestClose?: () => void } | null>(null);
const newExpOpen = ref(false);

const { width: windowWidth, height: windowHeight } = useWindowSize();

// 折叠屏 / 平板横屏触发双侧边栏桌面态；所有设备竖屏及直板/阔直板手机保持移动端视图
const isFoldableOrTabletLandscape = computed(() => {
  if (!isMobileOS) return false;
  const w = windowWidth.value;
  const h = windowHeight.value;
  if (w <= h || w < 680) return false;
  const ratio = w / Math.max(1, h);
  return h >= 500 || (h >= 420 && ratio <= 1.78);
});
const isCompactAndroidDesktop = computed(
  () => isMobileOS && isFoldableOrTabletLandscape.value && windowWidth.value < 1060
);

// 三档响应式工作台布局：宽屏(>=1280px 或折叠/平板横屏)可拖拽分栏 / 半屏(桌面768-1279px)悬浮侧滑Sheet / 窄屏或手机竖屏
const isWideDesktop = computed(() =>
  isMobileOS ? isFoldableOrTabletLandscape.value : windowWidth.value >= 1280
);
const isDesktopSidebar = computed(() =>
  isMobileOS ? isFoldableOrTabletLandscape.value : windowWidth.value >= 1024
);
const isTabletOrDesktop = computed(() =>
  isMobileOS ? isFoldableOrTabletLandscape.value : windowWidth.value >= 768
);
const isTablet = computed(() => isTabletOrDesktop.value && !isWideDesktop.value);
const isMobile = computed(() => !isTabletOrDesktop.value);

const desktopLeftSidebarOpen = ref(true);
const sheetLeftSidebarOpen = ref(false);
const leftSidebarOpen = computed({
  get: () => (isDesktopSidebar.value ? desktopLeftSidebarOpen.value : sheetLeftSidebarOpen.value),
  set: (val: boolean) => {
    if (isDesktopSidebar.value) {
      desktopLeftSidebarOpen.value = val;
    } else {
      sheetLeftSidebarOpen.value = val;
    }
  },
});

const desktopLeftSidebarWidth = computed(() => (isCompactAndroidDesktop.value ? 204 : 244));
const streamMinPct = computed(() => (isCompactAndroidDesktop.value ? 40 : 45));
const inspectorMinPct = computed(() => {
  const sidebarW =
    isDesktopSidebar.value && desktopLeftSidebarOpen.value ? desktopLeftSidebarWidth.value : 0;
  const minAvail = isCompactAndroidDesktop.value ? 480 : 900;
  const availW = Math.max(minAvail, windowWidth.value - sidebarW);
  const minPx = isCompactAndroidDesktop.value ? 232 : 310;
  return Math.min(44, Math.max(24, Math.ceil((minPx / availW) * 100)));
});

const desktopInspectorOpen = ref(true);
const inspectorPanelRef = ref<InstanceType<typeof ResizablePanel> | null>(null);
const inspectorAnimating = ref(false);
let inspectorAnimTimer: number | null = null;

function triggerInspectorAnim() {
  inspectorAnimating.value = true;
  if (inspectorAnimTimer) window.clearTimeout(inspectorAnimTimer);
  inspectorAnimTimer = window.setTimeout(() => {
    inspectorAnimating.value = false;
  }, 270);
}

watch(desktopInspectorOpen, (open) => {
  const panel = inspectorPanelRef.value;
  if (!panel) return;
  if (open && panel.isCollapsed) {
    triggerInspectorAnim();
    panel.expand();
    const targetPct = Math.max(inspectorMinPct.value, Math.min(55, inspectorWidthPct.value));
    nextTick(() => panel.resize(targetPct));
  } else if (!open && !panel.isCollapsed) {
    triggerInspectorAnim();
    panel.collapse();
  }
});

const inspectorWidthPct = ref(34);
const uiPrefsLayoutKey = ref(0);
const sheetInspectorOpen = ref(false);
let uiPrefsLoaded = false;
let saveUiPrefsTimer: number | null = null;

function scheduleSaveUiPrefs() {
  if (!uiPrefsLoaded) return;
  if (saveUiPrefsTimer) window.clearTimeout(saveUiPrefsTimer);
  saveUiPrefsTimer = window.setTimeout(() => {
    const prefs: UiPrefs = {
      leftSidebarOpen: desktopLeftSidebarOpen.value,
      inspectorOpen: desktopInspectorOpen.value,
      inspectorWidthPct: inspectorWidthPct.value,
      themePreference: themePreference.value,
    };
    invoke("save_ui_prefs", { prefs }).catch(() => {});
  }, 150);
}

watch(
  [desktopLeftSidebarOpen, desktopInspectorOpen, inspectorWidthPct, themePreference],
  scheduleSaveUiPrefs
);

watch(resolvedTheme, () => {
  syncThemeVisuals(false);
});

const inspectorOpen = computed({
  get: () => (isWideDesktop.value ? desktopInspectorOpen.value : sheetInspectorOpen.value),
  set: (val: boolean) => {
    if (isWideDesktop.value) {
      desktopInspectorOpen.value = val;
    } else {
      sheetInspectorOpen.value = val;
    }
  },
});

const streamDefaultSize = computed(() =>
  Math.max(
    streamMinPct.value,
    Math.min(100 - inspectorMinPct.value, 100 - inspectorWidthPct.value)
  )
);
const inspectorDefaultSize = computed(() => 100 - streamDefaultSize.value);

function onPanelLayout(sizes: number[]) {
  if (inspectorAnimating.value || !desktopInspectorOpen.value) return;
  if (sizes.length >= 2 && sizes[1] >= inspectorMinPct.value) {
    inspectorWidthPct.value = Math.round(sizes[1]);
  }
}

const inspectorTab = ref<"observer" | "board">("observer");
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

const displayPrivateNotes = computed<Record<string, string>>(() => {
  if (!experiment.value) return {};
  if (!replayActive.value) return experiment.value.privateNotes ?? {};
  const turn = replayCurrentTurn.value;
  const matching = [...replaySnapshots.value]
    .reverse()
    .find((s) => s.turnNumber <= turn);
  return matching?.privateNotes ?? {};
});

const activePrivateNotesCount = computed(
  () =>
    Object.values(displayPrivateNotes.value).filter((v) => v && v.trim().length > 0)
      .length
);

const displayCompactedSummary = computed(() => {
  if (!experiment.value) return "";
  if (!replayActive.value) return experiment.value.compactedSummary ?? "";
  const turn = replayCurrentTurn.value;
  const matching = [...replaySnapshots.value]
    .reverse()
    .find((s) => s.turnNumber <= turn);
  if (matching) return matching.compactedSummary ?? "";
  return turn >= (experiment.value.totalTurns ?? 0)
    ? (experiment.value.compactedSummary ?? "")
    : "";
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

function handleGlobalKeydown(e: KeyboardEvent) {
  if (!(e.metaKey || e.ctrlKey)) return;
  if (e.key === ",") {
    e.preventDefault();
    settingsOpen.value = true;
  } else if (e.key.toLowerCase() === "n") {
    e.preventDefault();
    newExpOpen.value = true;
  } else if (e.key.toLowerCase() === "b") {
    e.preventDefault();
    leftSidebarOpen.value = !leftSidebarOpen.value;
  } else if (e.key === "\\") {
    e.preventDefault();
    inspectorOpen.value = !inspectorOpen.value;
  }
}

interface TextContextMenuState {
  open: boolean;
  x: number;
  y: number;
  canCut: boolean;
  canCopy: boolean;
  canPaste: boolean;
  canSelectAll: boolean;
  selectedText: string;
  targetEl: HTMLInputElement | HTMLTextAreaElement | null;
  selStart: number;
  selEnd: number;
}

const textContextMenu = ref<TextContextMenuState>({
  open: false,
  x: 0,
  y: 0,
  canCut: false,
  canCopy: false,
  canPaste: false,
  canSelectAll: false,
  selectedText: "",
  targetEl: null,
  selStart: 0,
  selEnd: 0,
});

function closeTextContextMenu() {
  if (textContextMenu.value.open) {
    textContextMenu.value.open = false;
  }
}

function isEditableTextControl(node: EventTarget | null): node is HTMLInputElement | HTMLTextAreaElement {
  if (node instanceof HTMLTextAreaElement) {
    return !node.disabled && !node.readOnly;
  }
  if (node instanceof HTMLInputElement) {
    const editableTypes = new Set(["text", "search", "url", "tel", "email", "password", "number"]);
    return editableTypes.has(node.type || "text") && !node.disabled && !node.readOnly;
  }
  return false;
}

function computeContextMenuPos(
  rawX: number,
  rawY: number,
  itemCount: number,
  fallbackEl?: HTMLElement | null
) {
  const menuW = 164;
  const menuH = Math.max(40, itemCount * 32 + 10);
  let anchorX = rawX;
  let anchorY = rawY;
  if (anchorX === 0 && anchorY === 0 && fallbackEl) {
    const r = fallbackEl.getBoundingClientRect();
    anchorX = r.left + 14;
    anchorY = r.top + 14;
  }
  const x =
    anchorX + menuW + 8 <= window.innerWidth
      ? Math.max(8, anchorX)
      : Math.max(8, anchorX - menuW);
  const y =
    anchorY + menuH + 8 <= window.innerHeight
      ? Math.max(8, anchorY)
      : Math.max(8, anchorY - menuH);
  return { x, y };
}

function handleDesktopContextMenu(e: MouseEvent) {
  if (!isDesktopOS) return;
  e.preventDefault();

  const editable = isEditableTextControl(e.target) ? e.target : null;
  if (editable) {
    const start = editable.selectionStart ?? editable.value.length;
    const end = editable.selectionEnd ?? editable.value.length;
    const selectedText = editable.value.slice(start, end);
    const canCut = selectedText.length > 0;
    const canCopy = selectedText.length > 0;
    const canPaste = true;
    const canSelectAll = editable.value.length > 0;
    const itemCount = [canCut, canCopy, canPaste, canSelectAll].filter(Boolean).length;
    const pos = computeContextMenuPos(e.clientX, e.clientY, itemCount, editable);
    textContextMenu.value = {
      open: true,
      x: pos.x,
      y: pos.y,
      canCut,
      canCopy,
      canPaste,
      canSelectAll,
      selectedText,
      targetEl: editable,
      selStart: start,
      selEnd: end,
    };
    return;
  }

  const domSelected = window.getSelection()?.toString() ?? "";
  if (domSelected.trim().length > 0) {
    const pos = computeContextMenuPos(e.clientX, e.clientY, 1);
    textContextMenu.value = {
      open: true,
      x: pos.x,
      y: pos.y,
      canCut: false,
      canCopy: true,
      canPaste: false,
      canSelectAll: false,
      selectedText: domSelected,
      targetEl: null,
      selStart: 0,
      selEnd: 0,
    };
    return;
  }

  closeTextContextMenu();
}

function handleContextMenuPointerDown(e: MouseEvent) {
  if (!textContextMenu.value.open) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest("[data-text-context-menu]")) return;
  closeTextContextMenu();
}

async function execContextAction(action: "cut" | "copy" | "paste" | "selectAll") {
  const state = textContextMenu.value;
  closeTextContextMenu();
  const el = state.targetEl;

  if (action === "copy") {
    if (state.selectedText) {
      await writeClipboardText(state.selectedText).catch(() => {});
    }
    return;
  }

  if (!el) return;
  el.focus();

  if (action === "selectAll") {
    el.setSelectionRange(0, el.value.length);
    return;
  }

  if (action === "cut") {
    const cutText = el.value.slice(state.selStart, state.selEnd);
    if (cutText) {
      await writeClipboardText(cutText).catch(() => {});
      el.setRangeText("", state.selStart, state.selEnd, "start");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
    return;
  }

  if (action === "paste") {
    const pasted = await readClipboardText();
    if (pasted) {
      el.setRangeText(pasted, state.selStart, state.selEnd, "end");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }
}

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
  window.removeEventListener("contextmenu", handleDesktopContextMenu);
  window.removeEventListener("mousedown", handleContextMenuPointerDown, true);
  stopReplayTimer();
  resetStreamDeltaBuffer();
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

const relatedBranches = computed<ExperimentSummary[]>(() => {
  const cur = experiment.value;
  if (!cur) return [];
  const curBase = cur.title.split(/\s*·\s*(?:分支|改写重演)/)[0].trim();
  return experiments.value.filter((item) => {
    if (item.id === cur.id) return false;
    if (cur.parentExperimentId && item.id === cur.parentExperimentId) return true;
    if (item.parentExperimentId === cur.id) return true;
    if (cur.parentExperimentId && item.parentExperimentId === cur.parentExperimentId) return true;
    if (!cur.parentExperimentId && !item.parentExperimentId && curBase) {
      const otherBase = item.title.split(/\s*·\s*(?:分支|改写重演)/)[0].trim();
      if (otherBase === curBase && (item.title !== cur.title)) {
        return /·\s*(?:分支|改写重演)/.test(item.title) || /·\s*(?:分支|改写重演)/.test(cur.title);
      }
    }
    return false;
  });
});

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

function bumpActiveExperimentSummary(totalTurns?: number, totalTokens?: number) {
  const cur = experiment.value;
  if (!cur) return;
  const idx = experiments.value.findIndex((item) => item.id === cur.id);
  if (idx < 0) {
    refreshExperiments();
    return;
  }
  const nowIso = new Date().toISOString();
  const updated: ExperimentSummary = {
    ...experiments.value[idx],
    totalTurns: totalTurns ?? cur.totalTurns,
    totalTokens: totalTokens ?? cur.totalTokens,
    updatedAt: nowIso,
  };
  if (idx === 0) {
    experiments.value[0] = updated;
  } else {
    const next = experiments.value.slice();
    next.splice(idx, 1);
    next.unshift(updated);
    experiments.value = next;
  }
}

async function refreshExperiments() {
  experiments.value = await invoke<ExperimentSummary[]>("list_experiments");
}

let pendingStreamId: string | null = null;
let pendingContentDelta = "";
let pendingReasoningDelta = "";
let pendingReasoningElapsedMs: number | null = null;
let pendingReasoningStartedAt: number | null = null;
let streamRafId: number | null = null;
let lastStreamFlushAt = 0;

function resetStreamDeltaBuffer() {
  if (streamRafId !== null) {
    window.cancelAnimationFrame(streamRafId);
    streamRafId = null;
  }
  pendingStreamId = null;
  pendingContentDelta = "";
  pendingReasoningDelta = "";
  pendingReasoningElapsedMs = null;
  pendingReasoningStartedAt = null;
}

function flushStreamDeltas(force = false) {
  if (force && streamRafId !== null) {
    window.cancelAnimationFrame(streamRafId);
    streamRafId = null;
  }
  if (!streaming.value || streaming.value.messageId !== pendingStreamId) {
    pendingContentDelta = "";
    pendingReasoningDelta = "";
    pendingReasoningElapsedMs = null;
    return;
  }
  if (!force && isMobileOS) {
    const elapsed = performance.now() - lastStreamFlushAt;
    if (elapsed < 32) {
      if (streamRafId === null) {
        streamRafId = window.requestAnimationFrame(() => {
          streamRafId = null;
          flushStreamDeltas(false);
        });
      }
      return;
    }
  }
  if (pendingReasoningDelta) {
    if (streaming.value.reasoningStartedAt === null) {
      streaming.value.reasoningStartedAt = pendingReasoningStartedAt ?? performance.now();
    }
    streaming.value.reasoningContent += pendingReasoningDelta;
    pendingReasoningDelta = "";
  }
  if (pendingReasoningElapsedMs !== null) {
    streaming.value.reasoningElapsedMs = Math.max(
      streaming.value.reasoningElapsedMs,
      pendingReasoningElapsedMs
    );
    pendingReasoningElapsedMs = null;
  }
  if (pendingContentDelta) {
    streaming.value.content += pendingContentDelta;
    pendingContentDelta = "";
  }
  lastStreamFlushAt = performance.now();
}

function scheduleStreamDeltaFlush() {
  if (streamRafId !== null) return;
  streamRafId = window.requestAnimationFrame(() => {
    streamRafId = null;
    flushStreamDeltas(false);
  });
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
        resetStreamDeltaBuffer();
        streaming.value = null;
        pendingApproval.value = null;
      }
      if (ev.reason) {
        showToast(ev.reason);
      }
      break;

    case "message_added":
      experiment.value.messages.push(ev.message);
      bumpActiveExperimentSummary();
      break;

    case "stream_start":
      resetStreamDeltaBuffer();
      pendingStreamId = ev.messageId;
      lastStreamFlushAt = performance.now();
      contextUsagePct.value = ev.contextUsagePct;
      contextActiveTokens.value = ev.activeTokens;
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
        if (pendingReasoningStartedAt === null && streaming.value.reasoningStartedAt === null) {
          pendingReasoningStartedAt = now;
        }
        const started = streaming.value.reasoningStartedAt ?? pendingReasoningStartedAt ?? now;
        pendingReasoningDelta += ev.delta;
        pendingReasoningElapsedMs = Math.max(0, now - started);
        scheduleStreamDeltaFlush();
      }
      break;

    case "stream_delta":
      if (streaming.value && streaming.value.messageId === ev.messageId) {
        const started = streaming.value.reasoningStartedAt ?? pendingReasoningStartedAt;
        if (started !== null && streaming.value.content.length === 0 && pendingContentDelta.length === 0) {
          pendingReasoningElapsedMs = Math.max(
            pendingReasoningElapsedMs ?? streaming.value.reasoningElapsedMs,
            performance.now() - started
          );
        }
        pendingContentDelta += ev.delta;
        scheduleStreamDeltaFlush();
      }
      break;

    case "stream_end":
      resetStreamDeltaBuffer();
      streaming.value = null;
      experiment.value.messages.push(ev.message);
      experiment.value.totalTurns = ev.totalTurns;
      experiment.value.totalTokens = ev.totalTokens;
      bumpActiveExperimentSummary(ev.totalTurns, ev.totalTokens);
      break;

    case "observer_started":
      observerRunning.value = true;
      break;

    case "observer_completed":
      observerRunning.value = false;
      experiment.value.observerNotes.unshift(ev.note);
      if (ev.note.perturbationInjected) {
        showToast("检测到对话停滞，已自动触发场景事件");
      }
      break;

    case "observer_error":
      observerRunning.value = false;
      showToast(`局势分析异常: ${ev.error}`);
      break;

    case "observer_skipped":
      observerRunning.value = false;
      break;

    case "memory_compacted":
      experiment.value.compactedUntilSeq = ev.compactedUntilSeq;
      experiment.value.compactedSummary = ev.compactedSummary;
      contextUsagePct.value = ev.contextUsagePct;
      showToast(
        `早期对话已自动摘要（~${ev.tokensBefore.toLocaleString()} → ${ev.tokensAfter.toLocaleString()} tok）`
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

    case "private_note_updated":
      if (!experiment.value.privateNotes) {
        experiment.value.privateNotes = {};
      }
      if (ev.note) {
        experiment.value.privateNotes[ev.agentId] = ev.note;
      } else {
        delete experiment.value.privateNotes[ev.agentId];
      }
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
      resetStreamDeltaBuffer();
      streaming.value = null;
      showToast(ev.message);
      break;

    case "engine_notice":
      showToast(ev.message);
      break;

    case "config_hot_updated":
      experiment.value.config = ev.config;
      break;

    case "private_room_updated":
      experiment.value.activePrivateRoom = ev.activeRoom;
      experiment.value.roomMembers = ev.roomMembers;
      break;

    case "narrative_finale_reached":
      finalePromptVisible.value = true;
      showToast(`已达第 ${ev.totalTurns}/${ev.targetTurns} 轮目标，本局已收尾`);
      break;
  }
}

onMounted(async () => {
  if (isWindows) {
    document.documentElement.classList.add("win-acrylic");
  }
  if (isMobileOS) {
    document.documentElement.classList.add("mobile-os");
  }
  window.addEventListener("keydown", handleGlobalKeydown);
  window.addEventListener("contextmenu", handleDesktopContextMenu);
  window.addEventListener("mousedown", handleContextMenuPointerDown, true);
  const win = window as unknown as { __handleAndroidBack?: () => boolean };
  win.__handleAndroidBack = () => {
    if (promptModalOpen.value) {
      promptModalOpen.value = false;
      return true;
    }
    if (settingsOpen.value) {
      if (settingsDrawerRef.value?.requestClose) {
        settingsDrawerRef.value.requestClose();
      } else {
        settingsOpen.value = false;
      }
      return true;
    }
    if (scriptModalOpen.value) {
      if (scriptModalRef.value?.requestClose) {
        scriptModalRef.value.requestClose();
      } else {
        scriptModalOpen.value = false;
      }
      return true;
    }
    if (newExpOpen.value) {
      newExpOpen.value = false;
      return true;
    }
    if (forkTweakOpen.value) {
      forkTweakOpen.value = false;
      return true;
    }
    if (sheetLeftSidebarOpen.value) {
      sheetLeftSidebarOpen.value = false;
      return true;
    }
    if (sheetInspectorOpen.value) {
      sheetInspectorOpen.value = false;
      return true;
    }
    if (isMobile.value && mobileTab.value !== "stream") {
      mobileTab.value = "stream";
      return true;
    }
    if (replayActive.value) {
      exitReplay();
      return true;
    }
    return false;
  };
  const winIdle = window as unknown as {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
  };
  if (typeof winIdle.requestIdleCallback === "function") {
    winIdle.requestIdleCallback(() => warmupMarkdownParser(), { timeout: 600 });
  } else {
    window.setTimeout(() => warmupMarkdownParser(), isMobileOS ? 450 : 120);
  }

  const ch = new Channel<StreamEvent>();
  ch.onmessage = handleStreamEvent;

  // Fallback in case inline HTML script didn't show the window yet
  invoke("show_main_window").catch(() => {});

  // Consume pre-initiated IPC promises (started in bootstrap.ts before Vue mounted)
  await Promise.all([
    invoke("register_stream_channel", { channel: ch }),
    initialDataPromises.activeExperiment.then((activeExp) => {
      experiment.value = activeExp;
    }),
    initialDataPromises.experiments.then((list) => {
      experiments.value = list;
    }),
    initialDataPromises.engineState.then((state) => {
      engineState.value = state;
    }),
    initialDataPromises.presets.then((loadedPresets) => {
      presets.value = loadedPresets;
    }),
    initialDataPromises.characterCards.then((cards) => {
      characterCards.value = cards;
    }),
    initialDataPromises.apiProfiles
      .then((profiles) => {
        apiProfiles.value = profiles;
      })
      .catch(() => {
        apiProfiles.value = [];
      }),
    initialDataPromises.uiPrefs
      .then((prefs) => {
        desktopLeftSidebarOpen.value = prefs.leftSidebarOpen;
        const clampedWidth = Math.max(inspectorMinPct.value, Math.min(55, prefs.inspectorWidthPct));
        const needsRemount =
          prefs.inspectorOpen !== desktopInspectorOpen.value ||
          clampedWidth !== inspectorWidthPct.value;
        desktopInspectorOpen.value = prefs.inspectorOpen;
        inspectorWidthPct.value = clampedWidth;
        if (prefs.themePreference) {
          setThemePreference(prefs.themePreference, true);
        } else {
          syncThemeVisuals(true);
        }
        if (needsRemount) {
          uiPrefsLayoutKey.value += 1;
        }
        uiPrefsLoaded = true;
      })
      .catch(() => {
        uiPrefsLoaded = true;
      }),
  ]);

  if (apiProfiles.value.length === 0) {
    const seeded = seedApiProfilesFromConfig(experiment.value?.config ?? null);
    try {
      apiProfiles.value = await invoke<ApiProfile[]>("save_api_profiles", {
        profiles: seeded,
      });
    } catch {
      apiProfiles.value = seeded;
    }
  }
});

async function startEngine() {
  if (missingKeys.value) {
    settingsOpen.value = true;
    showToast("请先为所有参与对话的角色配置 API Key");
    return;
  }
  await invoke("start_engine");
}

async function pauseEngine() {
  await invoke("pause_engine");
}

async function stepEngine(targetAgentId?: string | null) {
  if (missingKeys.value) {
    settingsOpen.value = true;
    showToast("请先为所有参与对话的角色配置 API Key");
    return;
  }
  await invoke("step_engine", { targetAgentId: targetAgentId ?? null });
}

async function saveApiProfiles(nextProfiles: ApiProfile[]) {
  apiProfiles.value = await invoke<ApiProfile[]>("save_api_profiles", {
    profiles: nextProfiles,
  });
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
  finalePromptVisible.value = false;
  comparisonReport.value = "";
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  contextUsagePct.value = null;
  contextActiveTokens.value = 0;
  boardLastActor.value = "";
  boardLastReason.value = "";
  experiment.value = await invoke<ExperimentDetail>("switch_experiment", {
    experimentId: id,
  });
  await refreshExperiments();
}

async function createNewExperiment(title: string, config: ExperimentConfig) {
  exitReplay();
  finalePromptVisible.value = false;
  comparisonReport.value = "";
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  contextUsagePct.value = null;
  contextActiveTokens.value = 0;
  boardLastActor.value = "";
  boardLastReason.value = "";
  experiment.value = await invoke<ExperimentDetail>("create_experiment", {
    title,
    config,
  });
  await refreshExperiments();
}

async function handleFork(payload: { turnNumber: number; messageId?: string | null }) {
  if (!experiment.value) return;
  exitReplay();
  finalePromptVisible.value = false;
  comparisonReport.value = "";
  streaming.value = null;
  queueLen.value = 0;
  const forked = await invoke<ExperimentDetail>("fork_experiment", {
    sourceExpId: experiment.value.id,
    targetTurn: payload.turnNumber,
    targetMessageId: payload.messageId ?? null,
    editMessageId: null,
    editContent: null,
  });
  experiment.value = forked;
  await refreshExperiments();
  forkTweakOpen.value = true;
  showToast(`已开启平行宇宙分支「${forked.title}」`);
}

async function handleSaveForkTweak(payload: {
  title: string;
  config: ExperimentConfig;
  autoStep: boolean;
}) {
  if (!experiment.value) return;
  const updated = await invoke<ExperimentDetail>("update_experiment", {
    experimentId: experiment.value.id,
    title: payload.title,
    config: payload.config,
  });
  experiment.value = updated;
  forkTweakOpen.value = false;
  await refreshExperiments();
  showToast("分支变量已保存");
  if (payload.autoStep) {
    await stepEngine();
  }
}

async function handleEditAndReplay(payload: {
  messageId: string;
  turnNumber: number;
  content: string;
}) {
  if (!experiment.value) return;
  exitReplay();
  finalePromptVisible.value = false;
  comparisonReport.value = "";
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  const forked = await invoke<ExperimentDetail>("fork_experiment", {
    sourceExpId: experiment.value.id,
    targetTurn: payload.turnNumber,
    targetMessageId: payload.messageId,
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
  showToast(
    mode === "approval"
      ? "已开启白板人工审核"
      : mode === "guardian"
        ? "已开启防泄密审核"
        : "已切换为白板自动通过"
  );
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
  finalePromptVisible.value = false;
  streaming.value = null;
  queueLen.value = 0;
  pendingApproval.value = null;
  contextUsagePct.value = null;
  contextActiveTokens.value = 0;
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

async function handleDirectorCommand(instruction: string) {
  if (!experiment.value || directorBusy.value) return;
  directorBusy.value = true;
  try {
    const summary = await invoke<string>("execute_director_command", {
      experimentId: experiment.value.id,
      instruction,
    });
    showToast(`🎬 ${summary}`);
  } catch (err: any) {
    showToast(`导演指令解析失败: ${err?.toString?.() ?? err}`);
  } finally {
    directorBusy.value = false;
  }
}

async function handleStartPrivateRoom(memberIds: string[], turns: number) {
  if (!experiment.value) return;
  try {
    await invoke("start_private_room", {
      experimentId: experiment.value.id,
      memberIds,
      turns,
    });
    showToast(`已开启私密房间（${turns}轮），仅成员可见`);
  } catch (err: any) {
    showToast(`开启私密房间失败: ${err?.toString?.() ?? err}`);
  }
}

async function handleClosePrivateRoom() {
  if (!experiment.value) return;
  await invoke("close_private_room", {
    experimentId: experiment.value.id,
  });
}

async function handleSaveCharacterCard(
  input: string | { agentId: string; label?: string; systemPrompt?: string }
) {
  if (!experiment.value) return;
  const payload =
    typeof input === "string" ? { agentId: input } : input;
  try {
    characterCards.value = await invoke<CharacterCard[]>("save_agent_to_character_library", {
      experimentId: experiment.value.id,
      agentId: payload.agentId,
      labelOverride: payload.label ?? null,
      promptOverride: payload.systemPrompt ?? null,
    });
    showToast("角色已保存至角色库");
  } catch (err: any) {
    showToast(`存入角色库失败: ${err?.toString?.() ?? err}`);
  }
}

async function handleDeleteCharacterCard(cardId: string) {
  characterCards.value = await invoke<CharacterCard[]>("delete_character_card", {
    cardId,
  });
  showToast("已从角色库移除");
}

function openFinaleReport() {
  finalePromptVisible.value = false;
  inspectorOpen.value = true;
  inspectorTab.value = "observer";
  mobileTab.value = "observer";
  nextTick(() => {
    observerPanelRef.value?.openReportModal();
  });
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
    showToast("复盘报告已生成");
  } catch (err: any) {
    showToast(`生成复盘报告失败: ${err?.toString?.() ?? err}`);
  } finally {
    generatingReport.value = false;
  }
}

async function handleDistillImprints() {
  if (!experiment.value || distillingImprints.value) return;
  distillingImprints.value = true;
  try {
    const res = await invoke<DistillImprintsResponse>(
      "distill_memory_imprints_to_library",
      { experimentId: experiment.value.id }
    );
    characterCards.value = res.cards;
    showToast(
      `已提取 ${res.distilled.length} 位角色的经历记忆并保存`
    );
  } catch (err: any) {
    showToast(`提取经历记忆失败: ${err?.toString?.() ?? err}`);
  } finally {
    distillingImprints.value = false;
  }
}

async function handleUpdateCardImprints(payload: {
  cardId: string;
  memoryImprints: string[];
}) {
  try {
    characterCards.value = await invoke<CharacterCard[]>(
      "update_character_card_imprints",
      {
        cardId: payload.cardId,
        memoryImprints: payload.memoryImprints,
      }
    );
    showToast("经历记忆已更新");
  } catch (err: any) {
    showToast(`更新经历记忆失败: ${err?.toString?.() ?? err}`);
  }
}

async function handleGenerateComparison(otherExpId: string) {
  if (!experiment.value || !otherExpId || generatingComparison.value) return;
  generatingComparison.value = true;
  try {
    const report = await invoke<string>("generate_worldline_comparison_report", {
      expIdA: experiment.value.id,
      expIdB: otherExpId,
    });
    comparisonReport.value = report;
    showToast("世界线分歧对照报告已生成");
  } catch (err: any) {
    showToast(`生成对照报告失败: ${err?.toString?.() ?? err}`);
  } finally {
    generatingComparison.value = false;
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
    const { save } = await import("@tauri-apps/plugin-dialog");
    const filePath = await save({
      defaultPath: `${safeName}.scenario.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (filePath) {
      await invoke("save_markdown_to_file", { path: filePath, content: jsonText });
      showToast("剧本模板已导出（已自动脱敏 API Key）");
    }
  } catch (err: any) {
    try {
      await writeClipboardText(jsonText);
      showToast("无法写入文件，已将剧本 JSON 复制到剪贴板");
    } catch {
      showToast(`剧本导出失败: ${err?.message ?? err}`);
    }
  }
}

async function handleImportScenario() {
  try {
    const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    const text = await invoke<string>("read_text_file", { path: selected });
    const raw = JSON.parse(text);
    const fallbackName = selected.split(/[\\/]/).pop()?.replace(/\.json$/i, "") || "导入剧本";
    const pkg: ScenarioPackage =
      raw && typeof raw === "object" && "config" in raw
        ? {
            version: typeof raw.version === "string" ? raw.version : "2.0",
            title:
              typeof raw.title === "string" && raw.title.trim()
                ? raw.title.trim()
                : fallbackName,
            config: raw.config,
            initialBoard:
              typeof raw.initialBoard === "string"
                ? raw.initialBoard
                : (raw.config?.initialBoard ?? ""),
          }
        : {
            version: "2.0",
            title: fallbackName,
            config: raw,
            initialBoard: raw?.initialBoard ?? "",
          };

    exitReplay();
    streaming.value = null;
    queueLen.value = 0;
    pendingApproval.value = null;
    contextUsagePct.value = null;
    contextActiveTokens.value = 0;
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
    const { save } = await import("@tauri-apps/plugin-dialog");
    const filePath = await save({
      defaultPath: `${safeName}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (filePath) {
      await invoke("save_markdown_to_file", { path: filePath, content: md });
      showToast("Markdown 实验档案已导出");
    }
  } catch (err: any) {
    try {
      await writeClipboardText(md);
      showToast("无法写入文件，已将完整 Markdown 实验档案复制到剪贴板");
    } catch {
      showToast(`Markdown 导出失败: ${err?.message ?? err}`);
    }
  }
}

async function exportPureMarkdown() {
  if (!experiment.value) return;
  const md = await invoke<string>("export_pure_script_markdown", {
    experimentId: experiment.value.id,
  });
  const safeName = experiment.value.title.replace(/[\\/:*?"<>|]/g, "_");
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const filePath = await save({
      defaultPath: `${safeName}_纯享剧本.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (filePath) {
      await invoke("save_markdown_to_file", { path: filePath, content: md });
      showToast("纯享剧本 Markdown (.md) 已导出");
    }
  } catch (err: any) {
    try {
      await writeClipboardText(md);
      showToast("无法写入文件，已将纯享剧本 Markdown 复制到剪贴板");
    } catch {
      showToast(`纯享剧本导出失败: ${err?.message ?? err}`);
    }
  }
}

async function exportPurePng() {
  if (!experiment.value) return;
  try {
    showToast("正在渲染 Apple 书卷风纯享剧本长图...");
    const bytes = await renderPureScriptPngBytes(experiment.value);
    const safeName = experiment.value.title.replace(/[\\/:*?"<>|]/g, "_");
    const { save } = await import("@tauri-apps/plugin-dialog");
    const filePath = await save({
      defaultPath: `${safeName}_纯享剧本.png`,
      filters: [{ name: "PNG Image", extensions: ["png"] }],
    });
    if (filePath) {
      await invoke("save_binary_to_file", {
        path: filePath,
        data: Array.from(bytes),
      });
      showToast("纯享剧本长图 (.png) 已导出");
    }
  } catch (err: any) {
    showToast(`导出纯享长图失败: ${err?.toString?.() ?? err}`);
  }
}

async function exportPurePdf() {
  if (!experiment.value) return;
  window.dispatchEvent(new Event("terrarium:prepare-print"));
  await nextTick();
  window.print();
  showToast("已打开系统打印 / 导出 PDF 视图");
}

async function handleJumpToTurn(turnNumber: number) {
  mobileTab.value = "stream";
  await nextTick();
  setTimeout(() => {
    streamRef.value?.scrollToTurn(turnNumber);
  }, 80);
}

async function handleInspectPrompt(messageId: string) {
  try {
    const raw = await invoke<string | null>("get_message_raw_prompt", { messageId });
    if (!raw) {
      showToast("该轮消息未记录原始提示词（可能为早期历史存档）");
      return;
    }
    promptModalRaw.value = raw;
    try {
      promptModalParsed.value = JSON.parse(raw) as RawPromptPayload;
    } catch {
      promptModalParsed.value = null;
    }
    promptCopied.value = false;
    promptModalOpen.value = true;
  } catch (err: any) {
    showToast(`读取原始提示词失败: ${err?.toString?.() ?? err}`);
  }
}

async function copyRawPrompt() {
  if (!promptModalRaw.value) return;
  try {
    const pretty = promptModalParsed.value
      ? JSON.stringify(promptModalParsed.value, null, 2)
      : promptModalRaw.value;
    await writeClipboardText(pretty);
    promptCopied.value = true;
    setTimeout(() => {
      promptCopied.value = false;
    }, 2000);
  } catch {
    showToast("复制失败，请手动选中复制");
  }
}
</script>

<template>
  <div
    id="app-main-shell"
    class="relative h-dvh w-screen flex overflow-hidden text-[#1D1D1F]"
    :class="[
      isWindows ? 'bg-transparent' : isMobileOS && isDesktopSidebar && desktopLeftSidebarOpen ? 'bg-[#F9F9FB]' : 'bg-white',
      isMobileOS ? 'pl-[var(--safe-left,0px)] pr-[var(--safe-right,0px)]' : '',
    ]"
  >
    <!-- Persistent Fixed Left Sidebar Toggle on Desktop (never shifts on open/collapse) -->
    <div
      v-if="isDesktopSidebar && !isWindows"
      class="pointer-events-none absolute top-0 left-0 z-30 h-[46px] flex items-center"
      :class="isMacOS ? 'pl-[80px]' : 'pl-3'"
    >
      <button
        type="button"
        @click="triggerHaptic('light'); leftSidebarOpen = !leftSidebarOpen"
        class="pointer-events-auto w-8 h-8 flex items-center justify-center rounded-lg text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05] active:bg-black/[0.08] transition-colors outline-none"
        title="展开/收起左侧实验档案栏 (Ctrl+B)"
      >
        <PanelLeft class="w-4 h-4 stroke-[1.75]" />
      </button>
    </div>
    <!-- Full-height Native Left Archive Sidebar (Desktop >= 1024px or Foldable/Tablet Landscape) -->
    <aside
      v-if="isDesktopSidebar"
      class="shrink-0 h-full overflow-hidden transition-[width,opacity] duration-250 ease-[cubic-bezier(0.22,1,0.36,1)]"
      :class="
        desktopLeftSidebarOpen
          ? isCompactAndroidDesktop
            ? 'w-[204px] opacity-100'
            : 'w-[244px] opacity-100'
          : 'w-0 opacity-0 pointer-events-none'
      "
      :aria-hidden="!desktopLeftSidebarOpen"
    >
      <div
        class="h-full transition-transform duration-250 ease-[cubic-bezier(0.22,1,0.36,1)]"
        :class="[
          isCompactAndroidDesktop ? 'w-[204px]' : 'w-[244px]',
          desktopLeftSidebarOpen ? 'translate-x-0' : '-translate-x-3',
        ]"
      >
        <LeftSidebar
          :experiments="experiments"
          :current-experiment-id="experiment?.id ?? null"
          :current-total-turns="experiment?.totalTurns ?? 0"
          :distilling-imprints="distillingImprints"
          :character-cards="characterCards"
          @open-new-experiment="newExpOpen = true"
          @select-experiment="selectExperiment"
          @delete-experiment="deleteExperiment"
          @export-scenario="handleExportScenario"
          @import-scenario="handleImportScenario"
          @export-md="exportMarkdown"
          @export-pure-md="exportPureMarkdown"
          @export-pure-png="exportPurePng"
          @export-pure-pdf="exportPurePdf"
          @distill-imprints="handleDistillImprints"
          @update-card-imprints="handleUpdateCardImprints"
          @delete-character-card="handleDeleteCharacterCard"
          @open-settings="settingsOpen = true"
        />
      </div>
    </aside>

    <!-- Left Archive Slide-out Sheet (< 1024px) -->
    <Sheet
      v-if="!isDesktopSidebar"
      side="left"
      :open="sheetLeftSidebarOpen"
      title="实验档案库"
      width-class="w-[308px] max-w-[82vw]"
      @update:open="sheetLeftSidebarOpen = $event"
    >
      <LeftSidebar
        :experiments="experiments"
        :current-experiment-id="experiment?.id ?? null"
        :current-total-turns="experiment?.totalTurns ?? 0"
        :distilling-imprints="distillingImprints"
        :character-cards="characterCards"
        :in-sheet="true"
        @open-new-experiment="sheetLeftSidebarOpen = false; newExpOpen = true"
        @select-experiment="(id) => { sheetLeftSidebarOpen = false; selectExperiment(id); }"
        @delete-experiment="deleteExperiment"
        @export-scenario="handleExportScenario"
        @import-scenario="handleImportScenario"
        @export-md="exportMarkdown"
        @export-pure-md="exportPureMarkdown"
        @export-pure-png="exportPurePng"
        @export-pure-pdf="exportPurePdf"
        @distill-imprints="handleDistillImprints"
        @update-card-imprints="handleUpdateCardImprints"
        @delete-character-card="handleDeleteCharacterCard"
        @open-settings="sheetLeftSidebarOpen = false; settingsOpen = true"
      />
    </Sheet>

    <!-- Right Frame: Transparent Windows Titlebar Strip + Floating White Workspace Card -->
    <div class="flex-1 min-w-0 h-full flex flex-col overflow-hidden">
      <WindowsTitleBar
        v-if="isWindows"
        :show-brand="!(isDesktopSidebar && desktopLeftSidebarOpen)"
      />

      <div
        class="relative flex-1 min-h-0 flex flex-col overflow-hidden bg-white transition-[border-radius] duration-250 ease-[cubic-bezier(0.22,1,0.36,1)]"
        :class="
          isWindows
            ? isDesktopSidebar && desktopLeftSidebarOpen
              ? 'md:rounded-tl-[12px] md:border-t md:border-l md:border-black/[0.06] md:shadow-[0_1px_14px_rgba(0,0,0,0.03)]'
              : 'md:rounded-t-[12px] md:border-t md:border-black/[0.06] md:shadow-[0_1px_14px_rgba(0,0,0,0.03)]'
            : ''
        "
      >
        <!-- Persistent Single Right Inspector Toggle at Top-Right Corner on Wide Desktop (replaces duplicate toggle + X) -->
        <div
          v-if="isWideDesktop"
          class="pointer-events-none absolute top-0 right-3.5 z-30 h-[46px] flex items-center"
        >
          <button
            type="button"
            @click="triggerHaptic('light'); inspectorOpen = !inspectorOpen"
            class="pointer-events-auto relative w-8 h-8 flex items-center justify-center rounded-lg text-slate-500 hover:text-[#1D1D1F] hover:bg-black/[0.05] active:bg-black/[0.08] transition-colors outline-none"
            title="展开/收起右侧边栏 (Ctrl+\)"
          >
            <PanelRight class="w-4 h-4 stroke-[1.75]" />
            <span
              v-if="pendingApproval"
              class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-amber-500 animate-ping"
            ></span>
          </button>
        </div>

        <template v-if="!isWideDesktop">
        <TopControlBar
          :experiment="experiment"
          :engine-state="engineState"
          :status-reason="statusReason"
          :missing-keys="missingKeys"
          :replay-active="replayActive"
          :sidebar-open="leftSidebarOpen"
          :traffic-light-clearance="isMacOS && !(isDesktopSidebar && desktopLeftSidebarOpen)"
          :hide-sidebar-toggle="isDesktopSidebar && !isWindows"
          :inspector-open="inspectorOpen"
          :hide-inspector-toggle="false"
          :has-pending-approval="Boolean(pendingApproval)"
          :context-usage-pct="contextUsagePct"
          :context-active-tokens="contextActiveTokens"
          :compact-mobile="isMobile"
          :blind-guess-mode="blindGuessMode"
          @start="startEngine"
          @pause="pauseEngine"
          @step="stepEngine"
          @open-settings="settingsOpen = true"
          @open-script="scriptModalOpen = true"
          @rename-experiment="renameExperiment"
          @toggle-replay="toggleReplay"
          @toggle-blind-guess="blindGuessMode = !blindGuessMode"
          @toggle-sidebar="leftSidebarOpen = !leftSidebarOpen"
          @toggle-inspector="inspectorOpen = !inspectorOpen"
        />

        <!-- Mobile View Switcher (portrait phones & vertical foldables/tablets) unified into single header surface -->
        <div
          v-if="isMobile"
          class="shrink-0 px-3 pb-1.5 pt-1 bg-white"
        >
          <div class="grid grid-cols-3 p-1 rounded-xl bg-[#EFEFF4] gap-1">
            <button
              type="button"
              @click="triggerHaptic('light'); mobileTab = 'stream'"
              class="h-8 flex items-center justify-center gap-1 rounded-lg text-[12.5px] font-medium transition-all"
              :class="mobileTab === 'stream' ? 'bg-white text-[#1D1D1F] shadow-[0_1px_3px_rgba(0,0,0,0.08)]' : 'text-slate-600'"
            >
              <span>对话流</span>
              <span class="text-[11px] opacity-65 tabular-nums">
                ({{ replayActive ? replayCurrentTurn : (experiment?.totalTurns ?? 0) }})
              </span>
            </button>
            <button
              type="button"
              @click="triggerHaptic('light'); mobileTab = 'board'"
              class="relative h-8 flex items-center justify-center gap-1 rounded-lg text-[12.5px] font-medium transition-all"
              :class="mobileTab === 'board' ? 'bg-white text-[#1D1D1F] shadow-[0_1px_3px_rgba(0,0,0,0.08)]' : 'text-slate-600'"
            >
              <span>白板/备忘</span>
              <span
                v-if="activePrivateNotesCount > 0"
                class="text-[10.5px] text-violet-600 font-semibold tabular-nums"
              >
                ({{ activePrivateNotesCount }})
              </span>
              <span
                v-if="pendingApproval"
                class="w-1.5 h-1.5 rounded-full bg-amber-500 animate-ping"
              ></span>
            </button>
            <button
              type="button"
              @click="triggerHaptic('light'); mobileTab = 'observer'"
              class="h-8 flex items-center justify-center gap-1 rounded-lg text-[12.5px] font-medium transition-all"
              :class="mobileTab === 'observer' ? 'bg-white text-[#1D1D1F] shadow-[0_1px_3px_rgba(0,0,0,0.08)]' : 'text-slate-600'"
            >
              <span>局势分析</span>
              <span class="text-[11px] opacity-65 tabular-nums">({{ displayObserverNotes.length }})</span>
            </button>
          </div>
        </div>
      </template>

      <!-- Main Stage: Stream + Full-Height Right Inspector Sidebar -->
      <main
        @touchstart.passive="onMainTouchStart"
        @touchend.passive="onMainTouchEnd"
        class="flex-1 flex min-h-0 overflow-hidden"
      >

      <!-- 1. Wide Desktop (>= 1280px): Resizable Split Panels with Full-Height Right Sidebar -->
      <ResizablePanelGroup
        v-if="isWideDesktop"
        :key="uiPrefsLayoutKey"
        direction="horizontal"
        class="flex-1 min-w-0 h-full"
        @layout="onPanelLayout"
      >
        <ResizablePanel
          :default-size="desktopInspectorOpen ? streamDefaultSize : 100"
          :min-size="streamMinPct"
          :class="inspectorAnimating ? 'transition-[flex-grow] duration-250 ease-[cubic-bezier(0.22,1,0.36,1)]' : ''"
        >
          <section class="relative flex-1 h-full flex flex-col bg-white min-w-0">
            <TopControlBar
              :experiment="experiment"
              :engine-state="engineState"
              :status-reason="statusReason"
              :missing-keys="missingKeys"
              :replay-active="replayActive"
              :sidebar-open="leftSidebarOpen"
              :traffic-light-clearance="isMacOS && !(isDesktopSidebar && desktopLeftSidebarOpen)"
              :hide-sidebar-toggle="isDesktopSidebar && !isWindows"
              :inspector-open="inspectorOpen"
              :hide-inspector-toggle="true"
              :has-pending-approval="Boolean(pendingApproval)"
              :context-usage-pct="contextUsagePct"
              :context-active-tokens="contextActiveTokens"
              :compact-mobile="false"
              :blind-guess-mode="blindGuessMode"
              @start="startEngine"
              @pause="pauseEngine"
              @step="stepEngine"
              @open-settings="settingsOpen = true"
              @open-script="scriptModalOpen = true"
              @rename-experiment="renameExperiment"
              @toggle-replay="toggleReplay"
              @toggle-blind-guess="blindGuessMode = !blindGuessMode"
              @toggle-sidebar="leftSidebarOpen = !leftSidebarOpen"
              @toggle-inspector="inspectorOpen = !inspectorOpen"
            />

            <ThoughtStream
              ref="streamRef"
              :experiment-id="experiment?.id ?? null"
              :messages="displayMessages"
              :streaming="displayStreaming"
              :config="experiment?.config ?? null"
              :compacted-summary="displayCompactedSummary"
              :can-branch="canBranch"
              :blind-guess-mode="blindGuessMode"
              :room-members="experiment?.roomMembers ?? {}"
              :active-private-room="experiment?.activePrivateRoom ?? null"
              @fork="handleFork"
              @edit-and-replay="handleEditAndReplay"
              @inspect-prompt="handleInspectPrompt"
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
              :active-private-room="experiment?.activePrivateRoom ?? null"
              :director-busy="directorBusy"
              @submit="enqueueIntervention"
              @director-command="handleDirectorCommand"
              @start-private-room="handleStartPrivateRoom"
              @close-private-room="handleClosePrivateRoom"
              @resolve-approval="handleResolveApproval"
              @trigger-observer="triggerObserverNow"
            />
          </section>
        </ResizablePanel>

        <ResizableHandle
          class="transition-opacity duration-200"
          :class="desktopInspectorOpen ? 'opacity-100' : 'opacity-0 pointer-events-none !w-0 !mx-0'"
        />
        <ResizablePanel
          ref="inspectorPanelRef"
          :default-size="desktopInspectorOpen ? inspectorDefaultSize : 0"
          :min-size="inspectorMinPct"
          :max-size="55"
          :collapsible="true"
          :collapsed-size="0"
          :class="inspectorAnimating ? 'transition-[flex-grow] duration-250 ease-[cubic-bezier(0.22,1,0.36,1)]' : ''"
          @collapse="desktopInspectorOpen = false"
          @expand="desktopInspectorOpen = true"
        >
          <aside
            class="w-full h-full flex flex-col bg-white overflow-hidden transition-[opacity,transform] duration-220 ease-[cubic-bezier(0.22,1,0.36,1)]"
            :class="[
              isCompactAndroidDesktop ? 'min-w-0' : 'min-w-[280px]',
              desktopInspectorOpen ? 'opacity-100 translate-x-0' : 'opacity-0 translate-x-4 pointer-events-none',
            ]"
            :aria-hidden="!desktopInspectorOpen"
          >
              <div
                :data-tauri-drag-region="isMacOS ? '' : undefined"
                class="flex items-center justify-between gap-2 pl-3.5 pr-12 bg-white shrink-0 select-none"
                :class="
                  isMobileOS
                    ? 'pt-[var(--safe-top,env(safe-area-inset-top))] min-h-[calc(46px+var(--safe-top,0px))]'
                    : 'h-[46px]'
                "
              >
                <div class="flex items-center gap-1">
                  <button
                    type="button"
                    @click="inspectorTab = 'observer'"
                    class="h-7 px-2.5 rounded-lg flex items-center gap-1.5 text-[12.5px] transition-colors"
                    :class="
                      inspectorTab === 'observer'
                        ? 'bg-black/[0.055] font-semibold text-[#1D1D1F]'
                        : 'font-medium text-slate-400 hover:text-slate-700 hover:bg-black/[0.035]'
                    "
                  >
                    <Telescope class="w-3.5 h-3.5" />
                    <span>局势分析</span>
                    <span
                      v-if="displayObserverNotes.length > 0"
                      class="text-[11px] opacity-65 tabular-nums"
                    >
                      ({{ displayObserverNotes.length }})
                    </span>
                  </button>
                  <button
                    type="button"
                    @click="inspectorTab = 'board'"
                    class="relative h-7 px-2.5 rounded-lg flex items-center gap-1.5 text-[12.5px] transition-colors"
                    :class="
                      inspectorTab === 'board'
                        ? 'bg-black/[0.055] font-semibold text-[#1D1D1F]'
                        : 'font-medium text-slate-400 hover:text-slate-700 hover:bg-black/[0.035]'
                    "
                  >
                    <ClipboardList class="w-3.5 h-3.5" />
                    <span>共享白板</span>
                    <span
                      v-if="activePrivateNotesCount > 0"
                      class="px-1.5 py-0.5 rounded-full bg-violet-50 text-violet-700 text-[10.5px] font-semibold tabular-nums leading-none"
                      title="包含角色私密备忘"
                    >
                      备忘 {{ activePrivateNotesCount }}
                    </span>
                    <span
                      v-if="pendingApproval"
                      class="w-2 h-2 rounded-full bg-amber-500 animate-ping"
                    ></span>
                  </button>
                </div>
              </div>

              <div class="flex-1 min-h-0 overflow-hidden">
                <SharedBoardDrawer
                  v-show="inspectorTab === 'board'"
                  :open="true"
                  :embedded="true"
                  :board-content="displayBoardContent"
                  :board-prev-content="displayBoardPrevContent"
                  :approval-mode="experiment?.config.boardApprovalMode ?? 'auto'"
                  :last-actor="boardLastActor"
                  :last-reason="boardLastReason"
                  :read-only="replayActive"
                  :agents="experiment?.config.agents ?? []"
                  :private-notes="displayPrivateNotes"
                  :enable-private-notes="Boolean(experiment?.config.enablePrivateNotes)"
                  :blind-guess-mode="blindGuessMode"
                  @close="inspectorOpen = false"
                  @save-board="handleSaveBoard"
                  @toggle-approval-mode="handleToggleApprovalMode"
                />

                <ObserverPanel
                  ref="observerPanelRef"
                  v-show="inspectorTab === 'observer'"
                  :notes="displayObserverNotes"
                  :observer-running="observerRunning"
                  :observer-interval="experiment?.config.observerInterval ?? 10"
                  :total-turns="replayActive ? replayCurrentTurn : (experiment?.totalTurns ?? 0)"
                  :has-observer-key="hasObserverKey"
                  :final-report="experiment?.finalReport ?? ''"
                  :generating-report="generatingReport"
                  :agents="experiment?.config.agents ?? []"
                  :blind-guess-mode="blindGuessMode"
                  :related-branches="relatedBranches"
                  :distilling-imprints="distillingImprints"
                  :comparison-report="comparisonReport"
                  :generating-comparison="generatingComparison"
                  @jump-to-turn="handleJumpToTurn"
                  @trigger-now="triggerObserverNow"
                  @generate-report="handleGenerateReport"
                  @save-character-card="handleSaveCharacterCard"
                  @distill-imprints="handleDistillImprints"
                  @generate-comparison="handleGenerateComparison"
                />
              </div>
            </aside>
          </ResizablePanel>
      </ResizablePanelGroup>

      <!-- 2. Half-screen / Tablet (768px~1279px) & Mobile (< 768px) -->
      <template v-else>
        <section
          class="relative flex-1 h-full flex-col bg-white min-w-0"
          :class="isMobile && mobileTab !== 'stream' ? 'hidden' : 'flex'"
        >
          <ThoughtStream
            ref="streamRef"
            :experiment-id="experiment?.id ?? null"
            :messages="displayMessages"
            :streaming="displayStreaming"
            :config="experiment?.config ?? null"
            :compacted-summary="displayCompactedSummary"
            :can-branch="canBranch"
            :blind-guess-mode="blindGuessMode"
            :room-members="experiment?.roomMembers ?? {}"
            :active-private-room="experiment?.activePrivateRoom ?? null"
            @fork="handleFork"
            @edit-and-replay="handleEditAndReplay"
            @inspect-prompt="handleInspectPrompt"
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
            :active-private-room="experiment?.activePrivateRoom ?? null"
            :director-busy="directorBusy"
            @submit="enqueueIntervention"
            @director-command="handleDirectorCommand"
            @start-private-room="handleStartPrivateRoom"
            @close-private-room="handleClosePrivateRoom"
            @resolve-approval="handleResolveApproval"
            @trigger-observer="triggerObserverNow"
          />
        </section>

        <!-- Tablet / Half-screen Floating Sheet Inspector (768px ~ 1279px) -->
        <Sheet
          v-if="isTablet"
          :open="sheetInspectorOpen"
          title="局势分析与共享白板"
          width-class="w-[380px] min-[900px]:w-[420px] max-w-[86vw]"
          @update:open="sheetInspectorOpen = $event"
        >
          <div class="h-12 flex items-center justify-between gap-2 px-4 bg-white shrink-0">
            <div class="flex items-center p-0.5 rounded-xl bg-[#EFEFF4]">
              <button
                type="button"
                @click="triggerHaptic('light'); inspectorTab = 'observer'"
                class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-[12.5px] font-medium transition"
                :class="
                  inspectorTab === 'observer'
                    ? 'bg-white text-[#1D1D1F] shadow-[0_1px_2px_rgba(0,0,0,0.08)]'
                    : 'text-slate-600 hover:text-[#1D1D1F]'
                "
              >
                <Telescope class="w-3.5 h-3.5" />
                <span>局势分析</span>
                <span
                  v-if="displayObserverNotes.length > 0"
                  class="text-[11px] opacity-65 tabular-nums"
                >
                  ({{ displayObserverNotes.length }})
                </span>
              </button>
              <button
                type="button"
                @click="triggerHaptic('light'); inspectorTab = 'board'"
                class="relative flex items-center gap-1.5 px-3 py-1 rounded-lg text-[12.5px] font-medium transition"
                :class="
                  inspectorTab === 'board'
                    ? 'bg-white text-[#1D1D1F] shadow-[0_1px_2px_rgba(0,0,0,0.08)]'
                    : 'text-slate-600 hover:text-[#1D1D1F]'
                "
              >
                <ClipboardList class="w-3.5 h-3.5" />
                <span>共享白板</span>
                <span
                  v-if="activePrivateNotesCount > 0"
                  class="px-1.5 py-0.5 rounded-full bg-violet-50 text-violet-700 text-[10px] font-semibold tabular-nums leading-none"
                >
                  备忘 {{ activePrivateNotesCount }}
                </span>
                <span
                  v-if="pendingApproval"
                  class="w-2 h-2 rounded-full bg-amber-500 animate-ping"
                ></span>
              </button>
            </div>

            <button
              type="button"
              @click="sheetInspectorOpen = false"
              class="w-8 h-8 flex items-center justify-center rounded-full bg-[#F2F2F7] text-slate-600 active:bg-[#E5E5EA] transition"
              title="关闭面板"
            >
              <X class="w-4 h-4" />
            </button>
          </div>

          <div class="flex-1 min-h-0 overflow-hidden">
            <SharedBoardDrawer
              v-show="inspectorTab === 'board'"
              :open="true"
              :embedded="true"
              :board-content="displayBoardContent"
              :board-prev-content="displayBoardPrevContent"
              :approval-mode="experiment?.config.boardApprovalMode ?? 'auto'"
              :last-actor="boardLastActor"
              :last-reason="boardLastReason"
              :read-only="replayActive"
              :agents="experiment?.config.agents ?? []"
              :private-notes="displayPrivateNotes"
              :enable-private-notes="Boolean(experiment?.config.enablePrivateNotes)"
              :blind-guess-mode="blindGuessMode"
              @close="sheetInspectorOpen = false"
              @save-board="handleSaveBoard"
              @toggle-approval-mode="handleToggleApprovalMode"
            />

            <ObserverPanel
              ref="observerPanelRef"
              v-show="inspectorTab === 'observer'"
              :notes="displayObserverNotes"
              :observer-running="observerRunning"
              :observer-interval="experiment?.config.observerInterval ?? 10"
              :total-turns="replayActive ? replayCurrentTurn : (experiment?.totalTurns ?? 0)"
              :has-observer-key="hasObserverKey"
              :final-report="experiment?.finalReport ?? ''"
              :generating-report="generatingReport"
              :agents="experiment?.config.agents ?? []"
              :blind-guess-mode="blindGuessMode"
              :related-branches="relatedBranches"
              :distilling-imprints="distillingImprints"
              :comparison-report="comparisonReport"
              :generating-comparison="generatingComparison"
              @jump-to-turn="handleJumpToTurn"
              @trigger-now="triggerObserverNow"
              @generate-report="handleGenerateReport"
              @save-character-card="handleSaveCharacterCard"
              @distill-imprints="handleDistillImprints"
              @generate-comparison="handleGenerateComparison"
            />
          </div>
        </Sheet>

        <!-- Mobile Full-Width Inspector Panel (< 768px) -->
        <aside
          v-else-if="mobileTab !== 'stream'"
          class="flex-1 h-full flex flex-col bg-white overflow-hidden"
        >
          <div class="flex-1 min-h-0 overflow-hidden">
            <SharedBoardDrawer
              v-show="mobileTab === 'board'"
              :open="true"
              :embedded="true"
              :board-content="displayBoardContent"
              :board-prev-content="displayBoardPrevContent"
              :approval-mode="experiment?.config.boardApprovalMode ?? 'auto'"
              :last-actor="boardLastActor"
              :last-reason="boardLastReason"
              :read-only="replayActive"
              :agents="experiment?.config.agents ?? []"
              :private-notes="displayPrivateNotes"
              :enable-private-notes="Boolean(experiment?.config.enablePrivateNotes)"
              :blind-guess-mode="blindGuessMode"
              @close="mobileTab = 'stream'"
              @save-board="handleSaveBoard"
              @toggle-approval-mode="handleToggleApprovalMode"
            />

            <ObserverPanel
              ref="observerPanelRef"
              v-show="mobileTab === 'observer'"
              :notes="displayObserverNotes"
              :observer-running="observerRunning"
              :observer-interval="experiment?.config.observerInterval ?? 10"
              :total-turns="replayActive ? replayCurrentTurn : (experiment?.totalTurns ?? 0)"
              :has-observer-key="hasObserverKey"
              :final-report="experiment?.finalReport ?? ''"
              :generating-report="generatingReport"
              :agents="experiment?.config.agents ?? []"
              :blind-guess-mode="blindGuessMode"
              :related-branches="relatedBranches"
              :distilling-imprints="distillingImprints"
              :comparison-report="comparisonReport"
              :generating-comparison="generatingComparison"
              @jump-to-turn="handleJumpToTurn"
              @trigger-now="triggerObserverNow"
              @generate-report="handleGenerateReport"
              @save-character-card="handleSaveCharacterCard"
              @distill-imprints="handleDistillImprints"
              @generate-comparison="handleGenerateComparison"
            />
          </div>
        </aside>
      </template>
    </main>
    </div>
    </div>

    <!-- Custom Lightweight Text Context Menu (Replaces WebView2 browser menu) -->
    <Teleport to="body">
      <div
        v-if="textContextMenu.open"
        data-text-context-menu
        :style="{ left: `${textContextMenu.x}px`, top: `${textContextMenu.y}px` }"
        class="pointer-events-auto fixed z-[200] w-[164px] py-1 rounded-xl bg-white/95 backdrop-blur-xl border border-black/[0.08] shadow-[0_10px_30px_rgba(0,0,0,0.16)] text-[12.5px] text-[#1D1D1F] select-none"
      >
        <button
          v-if="textContextMenu.canCut"
          type="button"
          @mousedown.prevent="execContextAction('cut')"
          class="w-full px-3 py-1.5 flex items-center justify-between hover:bg-black/[0.05] transition text-left"
        >
          <span class="flex items-center gap-2">
            <Scissors class="w-3.5 h-3.5 text-slate-500" />
            剪切
          </span>
          <span class="text-[10.5px] text-slate-400">Ctrl+X</span>
        </button>
        <button
          v-if="textContextMenu.canCopy"
          type="button"
          @mousedown.prevent="execContextAction('copy')"
          class="w-full px-3 py-1.5 flex items-center justify-between hover:bg-black/[0.05] transition text-left"
        >
          <span class="flex items-center gap-2">
            <Copy class="w-3.5 h-3.5 text-slate-500" />
            复制
          </span>
          <span class="text-[10.5px] text-slate-400">Ctrl+C</span>
        </button>
        <button
          v-if="textContextMenu.canPaste"
          type="button"
          @mousedown.prevent="execContextAction('paste')"
          class="w-full px-3 py-1.5 flex items-center justify-between hover:bg-black/[0.05] transition text-left"
        >
          <span class="flex items-center gap-2">
            <ClipboardPaste class="w-3.5 h-3.5 text-slate-500" />
            粘贴
          </span>
          <span class="text-[10.5px] text-slate-400">Ctrl+V</span>
        </button>
        <button
          v-if="textContextMenu.canSelectAll"
          type="button"
          @mousedown.prevent="execContextAction('selectAll')"
          class="w-full px-3 py-1.5 flex items-center justify-between hover:bg-black/[0.05] transition text-left"
        >
          <span class="flex items-center gap-2">
            <TextSelect class="w-3.5 h-3.5 text-slate-500" />
            全选
          </span>
          <span class="text-[10.5px] text-slate-400">Ctrl+A</span>
        </button>
      </div>
    </Teleport>

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

    <!-- Narrative Finale Prompt Banner -->
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 -translate-y-2"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="finalePromptVisible"
        class="fixed top-28 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 px-4 py-2 rounded-full bg-white/95 backdrop-blur-xl border border-black/[0.08] shadow-[0_12px_36px_rgba(0,0,0,0.14)] text-xs"
      >
        <span class="font-medium text-[#1D1D1F]">本局已达目标轮数</span>
        <button
          type="button"
          @click="openFinaleReport"
          class="px-3 py-1 rounded-full bg-[#007AFF] hover:bg-[#0062CC] text-white font-medium transition"
        >
          生成复盘报告
        </button>
        <button
          type="button"
          @click="finalePromptVisible = false"
          class="p-1 rounded-full hover:bg-slate-100 text-slate-400"
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>
    </Transition>

    <SettingsDrawer
      ref="settingsDrawerRef"
      :open="settingsOpen"
      :config="experiment?.config ?? null"
      :api-profiles="apiProfiles"
      @close="settingsOpen = false"
      @save="saveConfig"
      @save-api-profiles="saveApiProfiles"
      @notify="showToast"
    />

    <ScenarioScriptModal
      ref="scriptModalRef"
      :open="scriptModalOpen"
      :experiment-title="experiment?.title ?? ''"
      :config="experiment?.config ?? null"
      :character-cards="characterCards"
      :api-profiles="apiProfiles"
      @close="scriptModalOpen = false"
      @save="saveConfig"
      @save-preset="handleSavePreset"
      @delete-character-card="handleDeleteCharacterCard"
      @save-character-card="handleSaveCharacterCard"
      @notify="showToast"
    />

    <NewExperimentModal
      :open="newExpOpen"
      :presets="presets"
      :current-config="experiment?.config ?? null"
      :character-cards="characterCards"
      :api-profiles="apiProfiles"
      @close="newExpOpen = false"
      @create="createNewExperiment"
      @save-preset="handleSavePreset"
      @delete-preset="handleDeletePreset"
    />

    <ForkTweakModal
      :open="forkTweakOpen"
      :title="experiment?.title ?? ''"
      :config="experiment?.config ?? null"
      :character-cards="characterCards"
      :api-profiles="apiProfiles"
      @close="forkTweakOpen = false"
      @save-and-continue="handleSaveForkTweak"
    />

    <!-- Raw Prompt Inspector Adaptive Modal -->
    <AdaptiveModal
      :open="promptModalOpen"
      title="本轮完整提示词"
      max-width-class="max-w-3xl"
      mobile-presentation="fullscreen"
      @close="promptModalOpen = false"
    >
      <template #header>
        <div class="px-5 py-3.5 border-b border-black/[0.06] flex items-center justify-between gap-2 bg-[#FAFAFC] shrink-0">
          <div class="flex items-center gap-2 min-w-0">
            <div class="w-7 h-7 rounded-xl bg-indigo-50 text-indigo-600 flex items-center justify-center shrink-0">
              <Code2 class="w-4 h-4" />
            </div>
            <div class="min-w-0">
              <h3 class="text-xs font-semibold text-[#1D1D1F] truncate">本轮完整提示词</h3>
              <p v-if="promptModalParsed" class="text-[10.5px] text-slate-400 truncate font-mono">
                {{ promptModalParsed.model }} · temp {{ promptModalParsed.temperature }}
                <template v-if="promptModalParsed.reasoning_effort"> · effort {{ promptModalParsed.reasoning_effort }}</template>
                · {{ promptModalParsed.messages?.length ?? 0 }} 条上下文消息
              </p>
            </div>
          </div>

          <div class="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              @click="copyRawPrompt"
              class="flex items-center gap-1 px-3 py-1.5 rounded-full text-[11px] font-medium bg-[#F2F2F5] hover:bg-[#E5E5EA] text-slate-700 transition"
            >
              <Check v-if="promptCopied" class="w-3.5 h-3.5 text-emerald-600" />
              <Copy v-else class="w-3.5 h-3.5" />
              <span>{{ promptCopied ? "已复制 JSON" : "复制 JSON" }}</span>
            </button>
            <button
              type="button"
              @click="promptModalOpen = false"
              class="p-1.5 rounded-full hover:bg-slate-200/70 text-slate-400 hover:text-slate-700 transition"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>
      </template>

      <div class="flex-1 overflow-y-auto p-4 sm:p-5 space-y-3 bg-[#F6F6F8]">
        <template v-if="promptModalParsed?.messages">
          <div
            v-for="(msg, idx) in promptModalParsed.messages"
            :key="idx"
            class="rounded-2xl border overflow-hidden bg-white"
            :class="
              msg.role === 'system'
                ? 'border-indigo-200/90'
                : msg.role === 'assistant'
                  ? 'border-emerald-200/90'
                  : 'border-black/[0.07]'
            "
          >
            <div
              class="px-3.5 py-1.5 text-[10.5px] font-mono font-semibold uppercase tracking-wider border-b flex items-center justify-between"
              :class="
                msg.role === 'system'
                  ? 'bg-indigo-50/80 text-indigo-700 border-indigo-100'
                  : msg.role === 'assistant'
                    ? 'bg-emerald-50/80 text-emerald-700 border-emerald-100'
                    : 'bg-slate-50 text-slate-600 border-black/[0.05]'
              "
            >
              <span>#{{ idx + 1 }} · {{ msg.role }}</span>
              <span class="text-[10px] font-normal opacity-75">{{ (msg.content ?? "").length }} chars</span>
            </div>
            <pre class="p-3.5 text-xs leading-relaxed text-[#1D1D1F] whitespace-pre-wrap break-words font-mono overflow-x-auto">{{ msg.content ?? "" }}</pre>
          </div>

          <div
            v-if="promptModalParsed.private_note !== undefined && promptModalParsed.private_note !== null"
            class="rounded-2xl border border-violet-200/80 bg-white overflow-hidden"
          >
            <div class="px-3.5 py-1.5 text-[10.5px] font-mono font-semibold uppercase tracking-wider bg-violet-50/80 text-violet-800 border-b border-violet-100 flex items-center justify-between">
              <span>角色私密备忘（仅本人可见）</span>
              <span class="text-[10px] font-normal opacity-75">{{ promptModalParsed.private_note.length }} chars</span>
            </div>
            <pre class="p-3.5 text-xs leading-relaxed text-[#1D1D1F] whitespace-pre-wrap break-words font-mono overflow-x-auto">{{ promptModalParsed.private_note || "（当前私有便签为空）" }}</pre>
          </div>

          <div
            v-if="promptModalParsed.tools"
            class="rounded-2xl border border-amber-200/80 bg-white overflow-hidden"
          >
            <div class="px-3.5 py-1.5 text-[10.5px] font-mono font-semibold uppercase tracking-wider bg-amber-50/80 text-amber-800 border-b border-amber-100">
              本轮可用工具
            </div>
            <pre class="p-3.5 text-[11px] leading-relaxed text-slate-700 whitespace-pre-wrap break-words font-mono overflow-x-auto">{{ JSON.stringify(promptModalParsed.tools, null, 2) }}</pre>
          </div>
        </template>
        <pre
          v-else
          class="p-4 rounded-2xl bg-white border border-black/[0.06] text-xs font-mono whitespace-pre-wrap break-words text-slate-700"
        >{{ promptModalRaw }}</pre>
      </div>
    </AdaptiveModal>
  </div>

  <PureScriptPrintView :experiment="experiment" />
</template>
