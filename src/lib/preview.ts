import {
  activePhases,
  defaultSettings,
  emptyRun,
  type Learning,
  type Settings,
  type Snapshot,
  type Step,
  type Workflow,
} from "./types";
import { t } from "./i18n";
import { normalizeServerUrl } from "./utils";

// This adapter never captures the desktop, sends real input, or contacts a model server.
function stored<T>(key: string, fallback: T): T {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "null") ?? fallback;
  } catch {
    return fallback;
  }
}
let workflows = stored<(Workflow & { memory?: string })[]>(
  "klickwerk-preview-workflows",
  [],
).map((w) => ({
  id: w.id,
  name: w.name,
  prompt: [w.prompt, w.memory].filter(Boolean).join("\n\n"),
  updated_at: w.updated_at,
}));
const settings = {
  ...defaultSettings,
  ...stored<Partial<Settings>>("klickwerk-preview-settings", {}),
  api_key: "",
};
let state: Snapshot = {
  settings,
  has_api_key: false,
  config_path: "Browser preview",
  config_error: null,
  run: structuredClone(emptyRun),
  platform: "preview",
  locale: navigator.language.toLowerCase().startsWith("de") ? "de" : "en",
  workflows: [],
  workflow_error: null,
};
const drafts = new Map<
  string,
  {
    runId: number | null;
    sourceSteps: number;
    correction: string;
    workflow: Workflow;
    sourceUpdated?: number;
  }
>();
let timer: ReturnType<typeof setTimeout> | undefined;
let generation = 0;
let memory = "";
function summaries() {
  state.workflows = workflows.map((w) => ({
    id: w.id,
    name: w.name,
    prompt: w.prompt,
    updated_at: w.updated_at,
  }));
}
function emit() {
  summaries();
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
function note(actor: Step["actor"], description: string): Step {
  return {
    id: state.run.steps.length + 1,
    actor,
    description,
    action: null,
    status: "recorded",
    elapsed_ms: state.run.elapsed_ms,
    image_size: null,
    desktop_points: [],
  };
}
function persist(next: Workflow[]) {
  localStorage.setItem("klickwerk-preview-workflows", JSON.stringify(next));
  workflows = next;
  emit();
}
export async function previewCall<T>(
  command: string,
  args: Record<string, unknown>,
): Promise<T> {
  switch (command) {
    case "bootstrap":
      summaries();
      return structuredClone(state) as T;
    case "save_settings": {
      const next = {
        ...(args.settings as Settings),
        api_key: "",
        base_url: normalizeServerUrl(
          (args.settings as Settings).base_url,
        ).href.replace(/\/$/, ""),
      };
      localStorage.setItem("klickwerk-preview-settings", JSON.stringify(next));
      state = { ...state, settings: next, has_api_key: false };
      emit();
      return structuredClone(state) as T;
    }
    case "list_models":
      normalizeServerUrl((args.settings as Settings).base_url);
      return {
        models: ["qwen3-vl:8b", "gemma3:12b", "your-vision-model"],
        partial: false,
      } as T;
    case "test_connection":
      await new Promise((resolve) => setTimeout(resolve, 200));
      return "Preview connection looks good. No server was contacted." as T;
    case "start_task": {
      if (activePhases.includes(state.run.phase))
        throw new Error("A task is already running.");
      const previous = state.run;
      const resume = args.resumeRunId != null;
      if (
        resume &&
        (args.resumeRunId !== previous.id ||
          args.task !== previous.task ||
          !["stopped", "waiting", "error", "done"].includes(previous.phase) ||
          (["waiting", "done"].includes(previous.phase) &&
            !String(args.reply ?? "").trim()))
      ) {
        throw new Error(
          "This correction no longer belongs to the current task.",
        );
      }
      generation++;
      clearTimeout(timer);
      drafts.clear();
      if (resume) {
        const reply = String(args.reply ?? "").trim();
        if (reply) {
          if (!(
            previous.steps.at(-1)?.actor === "user" &&
            previous.steps.at(-1)?.description === reply
          ))
            previous.steps.push(note("user", reply));
        } else
          previous.steps.push(
            note(
              "system",
              "The user chose to continue without a correction. Verify the current desktop before continuing.",
            ),
          );
      } else {
        if (args.workflowId && !workflows.some((w) => w.id === args.workflowId))
          throw new Error("This workflow is no longer available.");
        memory = workflows.find((w) => w.id === args.workflowId)?.prompt ?? "";
      }
      state.run = {
        ...structuredClone(emptyRun),
        id: resume ? previous.id : generation,
        task: String(args.task),
        workflow_id: resume
          ? previous.workflow_id
          : ((args.workflowId as string | null) ?? null),
        steps: resume ? previous.steps : [],
        elapsed_ms: resume ? previous.elapsed_ms : 0,
        phase: "countdown",
        message:
          "Starting in 2 seconds. Move your mouse or press any key to interrupt.",
      };
      emit();
      later(() => {
        state.run.phase = "running";
        state.run.message = "Taking a look at your desktop…";
        state.run.elapsed_ms += 2000;
        later(() => {
          state.run.steps.push({
            ...note("agent", "Focus the document editor"),
            action: { type: "click", x: 420, y: 280, button: "left" },
            status: "completed",
            image_size: [1280, 720],
            desktop_points: [[630, 420]],
          });
          state.run.message = "Writing your document…";
          state.run.elapsed_ms += 700;
          later(() => {
            const scenario = new URLSearchParams(location.search).get(
              "scenario",
            );
            if (scenario === "ask" && !args.reply) {
              state.run.phase = "waiting";
              state.run.question =
                "Which folder should I use for the new document?";
              state.run.message = "Control is paused while you reply.";
              state.run.steps.push({
                ...note("agent", "Ask which folder to use"),
                action: { type: "ask_user", question: state.run.question },
              });
            } else if (scenario === "error") {
              state.run.phase = "error";
              state.run.message =
                "Cannot reach the model server. Check that it is running and try again.";
            } else {
              state.run.steps.push({
                ...note("agent", "Type the welcome note"),
                action: {
                  type: "text",
                  text: "Welcome to the team!\nGrüße 世界",
                },
                status: "completed",
              });
              state.run.phase = "done";
              state.run.message = "Your task is complete.";
              state.run.result =
                "Preview complete. Your desktop has not been changed.";
            }
            state.run.elapsed_ms += 900;
          }, 900);
        }, 700);
      }, 2000);
      return undefined as T;
    }
    case "stop_task":
      if (!activePhases.includes(state.run.phase)) return undefined as T;
      generation++;
      clearTimeout(timer);
      state.run.phase = "stopped";
      state.run.interrupted = true;
      state.run.message =
        "You took over. Tell me what to do differently, and I’ll use your correction when we continue.";
      state.run.steps.push(
        note(
          "system",
          "You took over with mouse or keyboard input. The last action may need correcting.",
        ),
      );
      emit();
      return undefined as T;
    case "get_workflow": {
      const item = workflows.find((w) => w.id === args.id);
      if (!item) throw new Error("This workflow is no longer available.");
      return structuredClone(item) as T;
    }
    case "prepare_workflow": {
      const runId = args.runId as number | null;
      if (
        activePhases.includes(state.run.phase) ||
        (runId != null && state.run.id !== runId)
      )
        throw new Error(
          "This session changed. Save again to include the latest changes.",
        );
      const sourceSteps = state.run.steps.length;
      const saved = workflows.find(
        (w) => w.id === (runId != null ? state.run.workflow_id : args.id),
      );
      if (runId == null && !saved)
        throw new Error("This workflow is no longer available.");
      const learning = args.learning as Learning;
      const corrections =
        runId != null
          ? state.run.steps
              .filter((s) => s.actor === "user")
              .map((s) => s.description)
          : [];
      const correction = String(args.correction ?? "").trim();
      if (correction) corrections.push(correction);
      await new Promise((resolve) => setTimeout(resolve, 450));
      const warning =
        new URLSearchParams(location.search).get("learning") === "error"
          ? "Cannot reach the model server. Check that it is running and try again."
          : null;
      if (
        activePhases.includes(state.run.phase) ||
        (runId != null &&
          (state.run.id !== runId || state.run.steps.length !== sourceSteps)) ||
        (saved &&
          !workflows.some(
            (w) => w.id === saved.id && w.updated_at === saved.updated_at,
          ))
      )
        throw new Error(
          "This session changed. Save again to include the latest changes.",
        );
      const previousPrompt = runId != null ? memory : "";
      const instructions = [
        ...new Set(
          [learning.prompt, previousPrompt, ...corrections].filter(Boolean),
        ),
      ].join("\n\n");
      const workflow: Workflow = {
        id: saved?.id ?? crypto.randomUUID(),
        name: ["My workflow", "Mein Workflow"].includes(learning.name)
          ? t("My desktop workflow")
          : learning.name,
        prompt: warning
          ? instructions
          : `${instructions}\n\n${t("Locate targets on the current desktop and verify each result before continuing.")}${runId != null && state.run.phase !== "done" ? ` ${t("The previous attempt was not verified as complete; check the last attempted action before repeating it.")}` : ""}`,
        updated_at: 0,
      };
      drafts.set(String(args.requestId), {
        runId,
        sourceSteps,
        correction,
        workflow,
        sourceUpdated: saved?.updated_at,
      });
      return {
        token: args.requestId,
        warning,
        workflow: structuredClone(workflow),
      } as T;
    }
    case "save_workflow": {
      const draft = drafts.get(String(args.token));
      if (!draft)
        throw new Error("This workflow draft is no longer available.");
      if (
        activePhases.includes(state.run.phase) ||
        (draft.runId != null &&
          (state.run.id !== draft.runId ||
            state.run.workflow_id !==
              (draft.sourceUpdated == null ? null : draft.workflow.id) ||
            state.run.steps.length !== draft.sourceSteps)) ||
        (draft.sourceUpdated != null &&
          !workflows.some(
            (w) =>
              w.id === draft.workflow.id &&
              w.updated_at === draft.sourceUpdated,
          ))
      )
        throw new Error(
          "This session changed. Save again to include the latest changes.",
        );
      const saved = { ...draft.workflow, updated_at: Date.now() };
      if (!saved.name.trim() || !saved.prompt.trim())
        throw new Error("Add a name and start prompt.");
      persist([...workflows.filter((w) => w.id !== saved.id), saved]);
      if (draft.runId != null) {
        if (
          draft.correction &&
          !(
            state.run.steps.at(-1)?.actor === "user" &&
            state.run.steps.at(-1)?.description === draft.correction
          )
        )
          state.run.steps.push(note("user", draft.correction));
        state.run.workflow_id = saved.id;
        memory = saved.prompt;
      }
      drafts.delete(String(args.token));
      emit();
      return structuredClone(saved) as T;
    }
    case "delete_workflow":
      if (activePhases.includes(state.run.phase))
        throw new Error("Take over before deleting workflows.");
      if (!workflows.some((w) => w.id === args.id))
        throw new Error("This workflow is no longer available.");
      persist(workflows.filter((w) => w.id !== args.id));
      // Deleting also releases the session's temporary history and saved association.
      state.run = structuredClone(emptyRun);
      memory = "";
      drafts.clear();
      emit();
      return undefined as T;
    case "reset_session":
      if (activePhases.includes(state.run.phase))
        throw new Error("Take over before starting a fresh session.");
      generation++;
      clearTimeout(timer);
      state.run = structuredClone(emptyRun);
      memory = "";
      drafts.clear();
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
