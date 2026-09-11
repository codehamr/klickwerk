import {
  activePhases,
  defaultSettings,
  emptyRun,
  type Learning,
  type Settings,
  type Snapshot,
  type Step,
  type Workflow,
  type SessionExport,
} from "./types";
import { downloadWorkflow, parseWorkflow, validateLearning } from "./workflows";
import { version } from "../../package.json";
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
    generation: number;
  }
>();
let timer: ReturnType<typeof setTimeout> | undefined;
let generation = 0;
let trainingTimer: ReturnType<typeof setInterval> | undefined;
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
function finishAttempt() {
  const run = state.run;
  const attempt = run.attempts.at(-1);
  if (!attempt) return;
  attempt.finished_at = Date.now();
  const last = run.steps.at(-1);
  attempt.last_step_id =
    last && last.id >= attempt.first_step_id ? last.id : null;
  attempt.phase = run.phase;
  attempt.message = run.message;
  attempt.result = run.result;
  attempt.question = run.question;
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
    case "resume_after_restart":
      return undefined as T;
    case "restart_as_administrator":
      throw new Error("Administrator restart is available in the Windows app.");
    case "list_models":
      normalizeServerUrl((args.settings as Settings).base_url);
      return {
        models: ["qwen3-vl:8b", "gemma3:12b", "your-vision-model"],
        partial: false,
      } as T;
    case "test_connection":
      await new Promise((resolve) => setTimeout(resolve, 200));
      return "Preview connection looks good. No server was contacted." as T;
    case "import_workflow":
      return parseWorkflow(String(args.contents)) as T;
    case "export_workflow": {
      const workflow = workflows.find((w) => w.id === args.id);
      if (!workflow) throw new Error("This workflow is no longer available.");
      return downloadWorkflow(workflow) as T;
    }
    case "start_training": {
      if (activePhases.includes(state.run.phase))
        throw new Error("A task is already running.");
      if (args.runId != null && args.runId !== state.run.id)
        throw new Error("This task is no longer available to save.");
      if (args.runId == null) {
        const saved = workflows.find((w) => w.id === args.workflowId);
        memory = saved?.prompt ?? "";
        state.run = {
          ...structuredClone(emptyRun),
          id: Date.now(),
          task: String(args.task),
          workflow_id: saved?.id ?? null,
          started_at: Date.now(),
        };
      }
      if (args.correction)
        state.run.steps.push(note("user", String(args.correction)));
      generation++;
      drafts.clear();
      state.run.phase = "training";
      state.run.training = {
        events: 0,
        screenshots: 0,
        elapsed_ms: 0,
        stop_reason: "",
      };
      state.run.interrupted = false;
      state.run.message =
        "Show the task in your apps. Finish with Ctrl + Shift + F9.";
      trainingTimer = setInterval(() => {
        if (state.run.phase !== "training" || !state.run.training) return;
        state.run.training.elapsed_ms += 500;
        state.run.training.events = Math.min(6, state.run.training.events + 1);
        emit();
      }, 500);
      emit();
      return undefined as T;
    }
    case "pause_training":
      if (!["training", "training_paused"].includes(state.run.phase))
        throw new Error("No demonstration is being recorded.");
      state.run.phase = args.paused ? "training_paused" : "training";
      emit();
      return undefined as T;
    case "start_task": {
      if (activePhases.includes(state.run.phase))
        throw new Error("A task is already running.");
      const previous = state.run;
      const resume = args.resumeRunId != null;
      const firstStepId = resume ? previous.steps.length + 1 : 1;
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
        started_at: resume ? previous.started_at : Date.now(),
        attempts: resume ? previous.attempts : [],
        message:
          "Starting in 2 seconds. Input interruption begins after the countdown.",
      };
      const currentSettings = state.settings;
      state.run.attempts.push({
        number: state.run.attempts.length + 1,
        started_at: Date.now(),
        finished_at: null,
        first_step_id: firstStepId,
        last_step_id: null,
        user_reply: resume ? String(args.reply ?? "") : null,
        warm_start_prompt: memory,
        phase: "running",
        message: "",
        result: "",
        question: "",
        settings: {
          base_url: currentSettings.base_url,
          model: currentSettings.model,
          screenshot_max_edge: currentSettings.screenshot_max_edge,
          request_timeout_seconds: currentSettings.request_timeout_seconds,
          max_steps: currentSettings.max_steps,
          language:
            currentSettings.language === "de" ||
            (currentSettings.language === "system" && state.locale === "de")
              ? "German"
              : "English",
        },
      });
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
            finishAttempt();
          }, 900);
        }, 700);
      }, 2000);
      return undefined as T;
    }
    case "stop_task":
      if (["training", "training_paused"].includes(state.run.phase)) {
        clearInterval(trainingTimer);
        state.run.phase = "stopped";
        state.run.message = "Demonstration recorded.";
        if (state.run.training)
          state.run.training.stop_reason = "Demonstration recorded.";
        emit();
        return undefined as T;
      }
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
      finishAttempt();
      emit();
      return undefined as T;
    case "export_session": {
      if (
        state.run.id !== args.runId ||
        !["done", "error", "stopped", "waiting"].includes(state.run.phase)
      )
        throw new Error("This session is no longer available to export.");
      const refinement =
        args.refinement == null ? null : String(args.refinement);
      if (refinement && new TextEncoder().encode(refinement).length > 16384)
        throw new Error("The refinement is too long to export.");
      const report: SessionExport = {
        schema_version: 3,
        evidence: {
          frames_observed: 0,
          frames_omitted: 0,
          frames: [],
          model_responses_observed: 0,
          model_responses_omitted: 0,
          model_responses: [],
        },
        exported_at: Date.now(),
        app: { name: "klickwerk", version, platform: "preview" },
        run: structuredClone(state.run),
        workflow: structuredClone(
          workflows.find((w) => w.id === state.run.workflow_id) ?? null,
        ),
        warm_start_prompt: memory,
        unsent_refinement: refinement?.trim() ? refinement : null,
        coverage: {
          history: "all_recorded_steps_and_attempts",
          screenshots: "unavailable_in_preview",
          raw_model_responses: "unavailable_in_preview",
          controller_revision: "preview",
          capture_backend: "unavailable_in_preview",
          clock: "timestamps_are_unix_ms",
        },
      };
      const filename = `klickwerk-session-${report.run.id}-${report.exported_at}.json`;
      const url = URL.createObjectURL(
        new Blob([JSON.stringify(report, null, 2) + "\n"], {
          type: "application/json",
        }),
      );
      const link = document.createElement("a");
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      return filename as T;
    }
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
      const learning = args.learning as Learning;
      validateLearning(learning);
      const revision = generation;
      const consolidate = args.consolidate !== false;
      const corrections =
        runId != null
          ? state.run.steps
              .filter((s) => s.actor === "user")
              .map((s) => s.description)
          : [];
      const correction = String(args.correction ?? "").trim();
      if (correction) corrections.push(correction);
      if (consolidate) await new Promise((resolve) => setTimeout(resolve, 450));
      const warning =
        consolidate &&
        new URLSearchParams(location.search).get("learning") === "error"
          ? "Cannot reach the model server. Check that it is running and try again."
          : null;
      if (
        revision !== generation ||
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
        prompt:
          warning || !consolidate
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
        generation: revision,
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
        draft.generation !== generation ||
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
      const saved = {
        ...draft.workflow,
        ...((args.learning as Learning | null) ?? {}),
        updated_at: Date.now(),
      };
      validateLearning(saved);
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
