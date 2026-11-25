export type SpeakerRole = "Ai" | "GodPublic" | "GodSystem" | "GodWhisper";

export type EngineState = "idle" | "running" | "paused" | "stepping";

export interface NodeConfig {
  id: string;
  label: string;
  baseUrl: string;
  apiKey: string;
  modelName: string;
  temperature: number;
  systemPrompt: string;
}

export interface PresetAgentPrompt {
  label: string;
  systemPrompt: string;
}

export interface PresetTemplate {
  id: string;
  name: string;
  description: string;
  globalSystemPrompt: string;
  initialTopic: string;
  agentPrompts: PresetAgentPrompt[];
  isBuiltin?: boolean;
  nodeALabel: string;
  nodeAPrompt: string;
  nodeBLabel: string;
  nodeBPrompt: string;
}

export type BoardApprovalMode = "auto" | "approval";

export interface ExperimentConfig {
  presetId: string;
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
  agents: NodeConfig[];
  nodeC: NodeConfig;
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

export interface ObserverNote {
  id: string;
  experimentId: string;
  triggerTurn: number;
  tags: string[];
  depthStage: string;
  note: string;
  tensionScore: number;
  abstractionScore: number;
  stagnationIndex: number;
  perturbationInjected: boolean;
  crisisCards: CrisisCard[];
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
  createdAt: string;
  updatedAt: string;
}

export interface TurnSnapshot {
  id: string;
  experimentId: string;
  turnNumber: number;
  messageIds: string[];
  boardContent: string;
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
  boardContent: string;
  boardPrevContent: string;
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
    };
