import {
  defaultSettings,
  emptyRun,
  type Settings,
  type Snapshot,
} from "./types";

// The browser adapter never captures the screen, contacts a provider, or sends desktop input.
const stored = localStorage.getItem("klickwerk-preview-settings");
let settings: Settings = defaultSettings;
try {
  if (stored)
    settings = { ...defaultSettings, ...JSON.parse(stored), api_key: "" };
} catch {
  /* Invalid preview data resets to defaults. */
}
let state: Snapshot = {
  settings,
  has_api_key: false,
  config_path: "Browser preview · settings stay in this browser",
  config_error: null,
  run: { ...emptyRun },
  platform: "preview",
};
let timer: ReturnType<typeof setTimeout> | undefined;
let generation = 0;
function emit() {
  window.dispatchEvent(
    new CustomEvent("state", { detail: structuredClone(state) }),
  );
}
function later(fn: () => void, ms: number) {
  const current = generation;
  timer = setTimeout(() => {
    if (current === generation) {
      fn();
      emit();
    }
  }, ms);
}

export async function previewCall<T>(
  command: string,
  args: Record<string, unknown>,
): Promise<T> {
  switch (command) {
    case "bootstrap":
      return structuredClone(state) as T;
    case "save_settings": {
      const next = { ...(args.settings as Settings), api_key: "" };
      new URL(next.base_url);
      state = { ...state, settings: next, has_api_key: false };
      localStorage.setItem("klickwerk-preview-settings", JSON.stringify(next));
      emit();
      return structuredClone(state) as T;
    }
    case "list_models":
      return {
        models: ["qwen3-vl:8b", "gemma3:12b", "your-vision-model"],
        partial: false,
      } as T;
    case "test_connection":
      await new Promise((resolve) => setTimeout(resolve, 450));
      return "Preview connection looks good. No server was contacted." as T;
    case "discover_servers":
      return [
        {
          name: "Preview server",
          base_url: "http://localhost:11434/v1",
          models: ["qwen3-vl:8b"],
        },
      ] as T;
    case "start_task":
    case "start_stop_test": {
      if (["checking", "countdown", "running"].includes(state.run.phase))
        throw new Error("A task is already running.");
      generation++;
      clearTimeout(timer);
      const isTest = command === "start_stop_test";
      state.run = {
        ...emptyRun,
        id: generation,
        task: isTest ? "Try the emergency stop" : String(args.task),
        phase: "countdown",
        message: "Starting in 3 seconds. Let go of your mouse and keyboard.",
      };
      emit();
      later(() => {
        state.run = {
          ...state.run,
          phase: "running",
          message: isTest
            ? "Stop test is running. No desktop input is sent."
            : "Taking a look at your desktop…",
          elapsed_ms: 3000,
        };
        if (isTest) return;
        later(() => {
          state.run.steps = [{ id: 1, description: "Found the right window" }];
          state.run.message = "Working on your task…";
          state.run.elapsed_ms = 4800;
          later(() => {
            const scenario = new URLSearchParams(location.search).get(
              "scenario",
            );
            if (scenario === "ask" && !args.reply) {
              state.run.phase = "waiting";
              state.run.question =
                "Which folder should I use for the new document?";
              state.run.message = "A quick question for you";
            } else if (scenario === "error") {
              state.run.phase = "error";
              state.run.message =
                "Cannot reach the model server. Check that it is running and try again.";
            } else {
              state.run.phase = "done";
              state.run.message = "All done";
              state.run.result =
                "This is a preview of a completed task. Your desktop has not been changed.";
              state.run.steps.push({
                id: 2,
                description: "Checked the result",
              });
            }
            state.run.elapsed_ms = 6500;
          }, 1700);
        }, 1800);
      }, 3000);
      return undefined as T;
    }
    case "stop_task":
      generation++;
      clearTimeout(timer);
      state.run = {
        ...state.run,
        phase: "stopped",
        message: "Stopped. Your mouse and keyboard are yours again.",
      };
      emit();
      return undefined as T;
    case "start_dictation":
      throw new Error(
        "Microphone dictation is available in the Windows app. You can type your task here.",
      );
    case "ui_heartbeat":
    case "cancel_request":
    case "stop_dictation":
      return undefined as T;
    default:
      throw new Error(`Unknown preview command: ${command}`);
  }
}
