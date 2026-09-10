import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Snapshot,
  Settings,
  Models,
  Discovery,
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
  discover: () => call<Discovery[]>("discover_servers"),
  cancelRequest: (requestId: string) =>
    call<void>("cancel_request", { requestId }),
  start: (task: string, reply?: string) =>
    call<void>("start_task", { task, reply: reply ?? null }),
  stop: () => call<void>("stop_task"),
  heartbeat: () => call<void>("ui_heartbeat"),
  stopTest: () => call<void>("start_stop_test"),
  speechStart: () => call<void>("start_dictation"),
  speechStop: () => call<void>("stop_dictation"),
  onState: (callback: (snapshot: Snapshot) => void) =>
    subscribe("state", callback),
  onSpeech: (callback: (update: SpeechUpdate) => void) =>
    subscribe("speech", callback),
};
