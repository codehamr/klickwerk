export interface Settings {
  version: number;
  base_url: string;
  model: string;
  api_key: string;
  screenshot_max_edge: number;
  request_timeout_seconds: number;
  max_steps: number;
  theme: "system" | "light" | "dark";
  reduce_motion: boolean;
}

export type Phase =
  | "idle"
  | "checking"
  | "countdown"
  | "running"
  | "waiting"
  | "stopped"
  | "done"
  | "error";
export type Action =
  | { type: "click"; x: number; y: number; button: "left" | "right" }
  | { type: "double_click" | "move"; x: number; y: number }
  | {
      type: "drag";
      x: number;
      y: number;
      x2: number;
      y2: number;
      duration_ms: number;
    }
  | { type: "scroll"; x: number; y: number; amount: number }
  | { type: "text"; text: string }
  | { type: "key"; key: string; modifiers: string[] }
  | { type: "wait"; duration_ms: number }
  | { type: "observe" }
  | { type: "ask_user"; question: string }
  | { type: "finish"; summary: string };
export interface Step {
  id: number;
  actor: "agent" | "user" | "system";
  description: string;
  action: Action | null;
  status:
    "pending" | "completed" | "interrupted" | "failed" | "skipped" | "recorded";
  elapsed_ms: number;
  image_size: [number, number] | null;
  desktop_points: [number, number][];
}
export interface Learning {
  name: string;
  prompt: string;
  memory: string;
}
export interface Workflow extends Learning {
  id: string;
  task: string;
  steps: Step[];
  updated_at: number;
}
export interface WorkflowSummary {
  id: string;
  name: string;
  prompt: string;
  corrections: number;
  updated_at: number;
}
export interface WorkflowDraft {
  token: string;
  workflow: Workflow;
}
export interface Run {
  id: number;
  phase: Phase;
  task: string;
  message: string;
  steps: Step[];
  result: string;
  question: string;
  elapsed_ms: number;
  interrupted: boolean;
  workflow_id: string | null;
}
export interface Snapshot {
  settings: Settings;
  has_api_key: boolean;
  config_path: string;
  config_error: string | null;
  run: Run;
  platform: "windows" | "preview";
  workflows: WorkflowSummary[];
  workflow_error: string | null;
}
export interface Models {
  models: string[];
  partial: boolean;
}
export interface SpeechUpdate {
  text: string;
  finished: boolean;
  error?: string;
}

export const activePhases: Phase[] = ["checking", "countdown", "running"];
export const emptyRun: Run = {
  id: 0,
  phase: "idle",
  task: "",
  message: "",
  steps: [],
  result: "",
  question: "",
  elapsed_ms: 0,
  interrupted: false,
  workflow_id: null,
};
export const defaultSettings: Settings = {
  version: 1,
  base_url: "http://localhost:11434/v1",
  model: "",
  api_key: "",
  screenshot_max_edge: 1280,
  request_timeout_seconds: 120,
  max_steps: 50,
  theme: "system",
  reduce_motion: false,
};
