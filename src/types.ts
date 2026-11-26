export type SpeakerRole = "Ai" | "GodPublic" | "GodSystem" | "GodWhisper";

export type EngineState = "idle" | "running" | "paused" | "stepping";

export type ThinkingLevel = "default" | "off" | "low" | "medium" | "high";

export interface NodeConfig {
  id: string;
  label: string;
  baseUrl: string;
  apiKey: string;
  modelName: string;
  temperature: number;
  thinkingLevel?: ThinkingLevel | string;
  systemPrompt: string;
}

export interface PresetAgentPrompt {
  label: string;
  systemPrompt: string;
}

export interface PresetConfigOverrides {
  allowPassTurn?: boolean;
  randomMoodDice?: boolean;
  intuitiveFastReply?: boolean;
  enableWhiteboard?: boolean;
  enablePrivateNotes?: boolean;
  smartTurnOrder?: boolean;
}

export interface PresetTemplate {
  id: string;
  name: string;
  description: string;
  globalSystemPrompt: string;
  initialTopic: string;
  tags?: string[];
  agentPrompts: PresetAgentPrompt[];
  isBuiltin?: boolean;
  recommendedConfig?: PresetConfigOverrides | null;
  nodeALabel: string;
  nodeAPrompt: string;
  nodeBLabel: string;
  nodeBPrompt: string;
}

export type BoardApprovalMode = "auto" | "guardian" | "approval";

export interface ExperimentConfig {
  presetId: string;
  tags?: string[];
  globalSystemPrompt?: string;
  initialTopic: string;
  observerInterval: number;
  pauseSeconds: number;
  maxConsecutiveTurns: number;
  tokenBudget?: number;
  contextWindowTokens?: number;
  compactionThresholdPct?: number;
  boardApprovalMode?: BoardApprovalMode;
  enableBoardTools?: boolean;
  initialBoard?: string;
  enablePrefixCacheOpt?: boolean;
  enableCompactionGuard?: boolean;
  enablePrivateNotes?: boolean;
  enableSmartSelector?: boolean;
  enableAdaptiveReasoning?: boolean;
  adaptiveLowReasoningEffort?: "minimal" | "low" | "medium" | string;
  enablePeriodicReflection?: boolean;
  enableAutoDirector?: boolean;
  enableNarrativeArc?: boolean;
  targetTotalTurns?: number;
  allowPassTurn?: boolean;
  randomMoodDice?: boolean;
  intuitiveFastReply?: boolean;
  agents: NodeConfig[];
  nodeC: NodeConfig;
}

export interface ActivePrivateRoom {
  roomId: string;
  memberIds: string[];
  remainingTurns: number;
  title?: string;
}

export interface CharacterCard {
  id: string;
  name: string;
  basePrompt: string;
  behaviorTraits: string;
  memoryImprints?: string[];
  sourceExperimentTitle: string;
  isBuiltin?: boolean;
  createdAt: string;
}

export interface DistilledImprint {
  agentId: string;
  agentLabel: string;
  imprint: string;
}

export interface DistillImprintsResponse {
  cards: CharacterCard[];
  distilled: DistilledImprint[];
}

export interface GeneratedAgentDraft {
  label: string;
  systemPrompt: string;
}

export interface GeneratedScenario {
  title: string;
  globalSystemPrompt: string;
  initialTopic: string;
  initialBoard: string;
  agents: GeneratedAgentDraft[];
}

export interface BoardApprovalRequest {
  experimentId: string;
  requestId: string;
  agentId: string;
  agentLabel: string;
  currentContent: string;
  proposedContent: string;
  reason: string;
}

export interface TimelineMessage {
  id: string;
  experimentId: string;
  seq: number;
  turnNumber: number;
  role: SpeakerRole;
  agentId: string | null;
  roomId?: string;
  reasoningContent: string;
  content: string;
  modelName: string;
  durationMs: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  whisperConsumed: boolean;
  createdAt: string;
}

export interface CrisisCard {
  title: string;
  kind: "GodSystem" | "GodPublic" | string;
  content: string;
}

export interface RelationshipEdge {
  from: string;
  to: string;
  trustScore: number;
  attitudeTag: string;
}

export interface ObserverNote {
  id: string;
  experimentId: string;
  triggerTurn: number;
  tags: string[];
  depthStage: string;
  narrativeAct?: string;
  note: string;
  tensionScore: number;
  abstractionScore: number;
  stagnationIndex: number;
  perturbationInjected: boolean;
  crisisCards: CrisisCard[];
  relationshipMatrix?: RelationshipEdge[];
  createdAt: string;
}

export interface ScenarioPackage {
  version: string;
  title: string;
  config: ExperimentConfig;
  initialBoard: string;
}

export interface ExperimentSummary {
  id: string;
  title: string;
  totalTurns: number;
  totalTokens: number;
  nextSpeaker: string;
  parentExperimentId?: string | null;
  forkTurn?: number | null;
  createdAt: string;
  updatedAt: string;
}

export interface TurnSnapshot {
  id: string;
  experimentId: string;
  turnNumber: number;
  messageIds: string[];
  boardContent: string;
  privateNotes?: Record<string, string>;
  activePrivateRoom?: ActivePrivateRoom | null;
  roomMembers?: Record<string, string[]>;
  compactedSummary: string;
  compactedUntilSeq: number;
  totalTokens: number;
  nextSpeaker: string;
  createdAt: string;
}

export interface ExperimentDetail {
  id: string;
  title: string;
  config: ExperimentConfig;
  totalTurns: number;
  totalTokens: number;
  nextSpeaker: string;
  parentExperimentId?: string | null;
  forkTurn?: number | null;
  boardContent: string;
  boardPrevContent: string;
  privateNotes?: Record<string, string>;
  activePrivateRoom?: ActivePrivateRoom | null;
  roomMembers?: Record<string, string[]>;
  compactedSummary: string;
  compactedUntilSeq: number;
  finalReport: string;
  messages: TimelineMessage[];
  observerNotes: ObserverNote[];
  createdAt: string;
  updatedAt: string;
}

export interface InterventionInput {
  kind: SpeakerRole;
  agentId?: string | null;
  content: string;
}

export type StreamEvent =
  | {
      type: "state_changed";
      experimentId: string;
      state: EngineState;
      reason: string | null;
      consecutiveTurns: number;
    }
  | {
      type: "message_added";
      experimentId: string;
      message: TimelineMessage;
    }
  | {
      type: "stream_start";
      experimentId: string;
      messageId: string;
      role: SpeakerRole;
      agentId: string;
      agentLabel: string;
      turnNumber: number;
      modelName: string;
      activeTokens: number;
      contextUsagePct: number;
    }
  | {
      type: "stream_delta";
      experimentId: string;
      messageId: string;
      delta: string;
    }
  | {
      type: "stream_reasoning_delta";
      experimentId: string;
      messageId: string;
      delta: string;
    }
  | {
      type: "stream_end";
      experimentId: string;
      message: TimelineMessage;
      totalTurns: number;
      totalTokens: number;
    }
  | {
      type: "observer_started";
      experimentId: string;
      triggerTurn: number;
    }
  | {
      type: "observer_completed";
      experimentId: string;
      note: ObserverNote;
    }
  | {
      type: "observer_error";
      experimentId: string;
      error: string;
    }
  | {
      type: "observer_skipped";
      experimentId: string;
      triggerTurn: number;
    }
  | {
      type: "memory_compacted";
      experimentId: string;
      compactedUntilSeq: number;
      compactedSummary: string;
      tokensBefore: number;
      tokensAfter: number;
      contextUsagePct: number;
    }
  | {
      type: "tool_call_started";
      experimentId: string;
      messageId: string;
      agentLabel: string;
      toolName: string;
    }
  | {
      type: "board_updated";
      experimentId: string;
      boardContent: string;
      boardPrevContent: string;
      reason: string;
      actorLabel: string;
    }
  | {
      type: "private_note_updated";
      experimentId: string;
      agentId: string;
      agentLabel: string;
      note: string;
    }
  | {
      type: "board_approval_requested";
      experimentId: string;
      requestId: string;
      agentId: string;
      agentLabel: string;
      currentContent: string;
      proposedContent: string;
      reason: string;
    }
  | {
      type: "board_approval_resolved";
      experimentId: string;
      requestId: string;
      approved: boolean;
    }
  | {
      type: "intervention_queued";
      experimentId: string;
      queueLen: number;
    }
  | {
      type: "engine_error";
      experimentId: string;
      message: string;
    }
  | {
      type: "engine_notice";
      experimentId: string;
      message: string;
    }
  | {
      type: "config_hot_updated";
      experimentId: string;
      config: ExperimentConfig;
    }
  | {
      type: "private_room_updated";
      experimentId: string;
      activeRoom: ActivePrivateRoom | null;
      roomMembers: Record<string, string[]>;
    }
  | {
      type: "narrative_finale_reached";
      experimentId: string;
      totalTurns: number;
      targetTurns: number;
    };

export interface DiffLine {
  type: "same" | "added" | "removed";
  text: string;
}

export type ThemePreference = "system" | "light" | "dark";

export interface UiPrefs {
  leftSidebarOpen: boolean;
  inspectorOpen: boolean;
  inspectorWidthPct: number;
  themePreference?: ThemePreference;
}
