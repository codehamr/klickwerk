import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Snapshot,
  Settings,
  Models,
  Workflow,
  WorkflowDraft,
  Learning,
  SpeechUpdate,
} from "./types";

export const native = isTauri();

export async function call<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (native) return invoke<T>(command, args);
  const { previewCall } = await import("./preview");
  return previewCall<T>(command, args);
}

export async function subscribe<T>(
  event: string,
  callback: (payload: T) => void,
): Promise<() => void> {
  if (native) return listen<T>(event, ({ payload }) => callback(payload));
  const handler = (e: Event) => callback((e as CustomEvent<T>).detail);
  window.addEventListener(event, handler);
  return () => window.removeEventListener(event, handler);
}

export const api = {
  bootstrap: () => call<Snapshot>("bootstrap"),
  save: (settings: Settings, key: string | null) =>
    call<Snapshot>("save_settings", { settings, key }),
  models: (settings: Settings, key: string | null, requestId: string) =>
    call<Models>("list_models", { settings, key, requestId }),
  test: (settings: Settings, key: string | null, requestId: string) =>
    call<string>("test_connection", { settings, key, requestId }),
  cancelRequest: (requestId: string) =>
    call<void>("cancel_request", { requestId }),
  start: (
    task: string,
    reply?: string,
    resumeRunId?: number,
    workflowId?: string,
  ) =>
    call<void>("start_task", {
      task,
      reply: reply ?? null,
      resumeRunId: resumeRunId ?? null,
      workflowId: workflowId ?? null,
    }),
  getWorkflow: (id: string) => call<Workflow>("get_workflow", { id }),
  prepareWorkflow: (
    learning: Learning,
    requestId: string,
    runId?: number,
    correction?: string,
    id?: string,
  ) =>
    call<WorkflowDraft>("prepare_workflow", {
      learning,
      requestId,
      runId: runId ?? null,
      correction: correction || null,
      id: id ?? null,
    }),
  saveWorkflow: (token: string) => call<Workflow>("save_workflow", { token }),
  reset: () => call<void>("reset_session"),
  deleteWorkflow: (id: string) => call<void>("delete_workflow", { id }),
  stop: () => call<void>("stop_task"),
  heartbeat: () => call<void>("ui_heartbeat"),
  speechStart: () => call<void>("start_dictation"),
  speechStop: () => call<void>("stop_dictation"),
  onState: (callback: (snapshot: Snapshot) => void) =>
    subscribe("state", callback),
  onSpeech: (callback: (update: SpeechUpdate) => void) =>
    subscribe("speech", callback),
};
