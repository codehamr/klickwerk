export interface Settings {
  version: number;
  base_url: string;
  model: string;
  api_key: string;
  screenshot_max_edge: number;
  request_timeout_seconds: number;
  max_steps: number;
  theme: "system" | "light" | "dark";
  language: "system" | "en" | "de";
}

export type Phase =
  | "training"
  | "training_paused"
  | "idle"
  | "recovering"
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
}
export interface Workflow extends Learning {
  id: string;
  updated_at: number;
}
export interface WorkflowSummary {
  id: string;
  name: string;
  prompt: string;
  updated_at: number;
}
export interface WorkflowDraft {
  token: string;
  warning?: string | null;
  workflow: Workflow;
}
export interface InputFailure {
  code: string;
  message: string;
  target: {
    window: {
      handle: number;
      process_id: number;
      title: string;
      class_name: string;
      executable: string;
      bounds: number[];
      integrity_level: number | null;
      elevated: boolean | null;
      inspection_error: number | null;
    };
    sender_integrity_level: number | null;
    input_block: string | null;
  } | null;
  win32_error: number | null;
  io_error_kind?: string;
  json_error?: { category: string; line: number; column: number };
}
export interface RecoveryEvent {
  recorded_at: number;
  kind: string;
  source: string;
  sender_integrity_level: number | null;
  failure: InputFailure | null;
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
  started_at: number;
  attempts: Attempt[];
  training?: TrainingSummary | null;
  recovery?: InputFailure | null;
  recovery_events?: RecoveryEvent[];
}
export interface TrainingOptions {
  text: boolean;
  screenshots: boolean;
}
export interface TrainingSummary {
  events: number;
  screenshots: number;
  elapsed_ms: number;
  stop_reason: string;
}
export interface TrainingReport {
  version: number;
  options: TrainingOptions;
  events: ({
    id: number;
    elapsed_ms: number;
    window: null | {
      handle: number;
      title: string;
      application: string;
      class_name: string;
      bounds: [number, number, number, number];
      focused_control: number;
    };
  } & (
    | { type: "focus" | "omitted_text" | "pause" | "resume" }
    | {
        type: "pointer";
        phase: string;
        button: string;
        position: [number, number];
      }
    | {
        type: "scroll";
        axis: string;
        delta: number;
        position: [number, number];
      }
    | { type: "text"; text: string }
    | { type: "key"; key: string; modifiers: string[] }
  ))[];
  screenshots: {
    after_event_id: number;
    elapsed_ms: number;
    bounds: [number, number, number, number];
    image_size: [number, number];
    jpeg_base64: string;
  }[];
  omitted_events: number;
  omitted_screenshots: number;
  elapsed_ms: number;
  stop_reason: string;
}
export interface Attempt {
  interruption?: {
    event: number;
    flags: number;
    detected_ms: number;
    since_agent_input_ms: number | null;
    position: [number, number] | null;
    anchor: [number, number] | null;
  };
  number: number;
  started_at: number;
  finished_at: number | null;
  first_step_id: number;
  last_step_id: number | null;
  user_reply: string | null;
  warm_start_prompt: string;
  phase: Phase;
  message: string;
  result: string;
  question: string;
  settings: Pick<
    Settings,
    | "base_url"
    | "model"
    | "screenshot_max_edge"
    | "request_timeout_seconds"
    | "max_steps"
  > & { language: string };
}
export interface SessionExport {
  schema_version: 3;
  exported_at: number;
  app: { name: string; version: string; platform: Snapshot["platform"] };
  run: Run;
  workflow: Workflow | null;
  warm_start_prompt: string;
  unsent_refinement: string | null;
  evidence: {
    training?: TrainingReport;
    frames_observed: number;
    frames_omitted: number;
    model_responses_observed: number;
    model_responses_omitted: number;
    model_responses: {
      frame_id: number;
      received_at: number;
      response_model: string | null;
      finish_reason: string | null;
      assistant_content: string | null;
      content_truncated: boolean;
      parse_error: string | null;
    }[];
    frames: {
      frame: Record<string, number>;
      observed_at: number;
      purpose: string;
      mime: string;
      jpeg_base64: string;
    }[];
  };
  coverage: {
    history: "all_recorded_steps_and_attempts";
    screenshots:
      "recent_frames_bounded_12_and_8_mib_base64" | "unavailable_in_preview";
    raw_model_responses:
      | "recent_assistant_text_bounded_12_and_32_kib_each"
      | "unavailable_in_preview";
    controller_revision: string;
    capture_backend: string;
    clock: string;
  };
}
export interface Snapshot {
  settings: Settings;
  has_api_key: boolean;
  config_path: string;
  config_error: string | null;
  run: Run;
  platform: "windows" | "preview";
  activity_busy?: boolean;
  locale: "en" | "de";
  workflows: WorkflowSummary[];
  workflow_error: string | null;
  can_restart_elevated?: boolean;
  restored_refinement?: string;
  pending_resume_run_id?: number | null;
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

export const activePhases: Phase[] = [
  "training",
  "training_paused",
  "recovering",
  "checking",
  "countdown",
  "running",
];
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
  started_at: 0,
  attempts: [],
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
  language: "system",
};
