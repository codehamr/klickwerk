export interface Settings {
  version: number;
  provider: "local" | "custom";
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
export interface Step {
  id: number;
  description: string;
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
}
export interface Snapshot {
  settings: Settings;
  has_api_key: boolean;
  config_path: string;
  config_error: string | null;
  run: Run;
  platform: "windows" | "preview";
}
export interface Models {
  models: string[];
  partial: boolean;
}
export interface Discovery {
  name: string;
  base_url: string;
  models: string[];
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
};
export const defaultSettings: Settings = {
  version: 1,
  provider: "local",
  base_url: "http://localhost:11434/v1",
  model: "",
  api_key: "",
  screenshot_max_edge: 1280,
  request_timeout_seconds: 120,
  max_steps: 50,
  theme: "system",
  reduce_motion: false,
};
