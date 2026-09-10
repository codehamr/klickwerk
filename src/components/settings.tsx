import { useI18n } from "../lib/i18n";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  Check,
  Eye,
  EyeOff,
  FolderLock,
  LoaderCircle,
  RefreshCw,
  SlidersHorizontal,
  Unplug,
  Wifi,
} from "lucide-react";
import { api } from "../lib/bridge";
import type { Settings as SettingsData, Snapshot } from "../lib/types";
import { errorText, normalizeServerUrl } from "../lib/utils";
import { Button } from "./ui/button";
import { Dialog, DialogContent } from "./ui/dialog";
import { Select } from "./ui/select";
import { ModelCombobox } from "./ui/model-combobox";

interface Props {
  snapshot: Snapshot;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (snapshot: Snapshot) => void;
  returnFocus: () => void;
}

export function SettingsDialog({
  snapshot,
  open,
  onOpenChange,
  onSaved,
  returnFocus,
}: Props) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<SettingsData>(snapshot.settings);
  const [key, setKey] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [tab, setTab] = useState<"connection" | "preferences">("connection");
  const [models, setModels] = useState<string[]>([]);
  const [notice, setNotice] = useState<{
    kind: "success" | "error" | "info";
    text: string;
  } | null>(null);
  const [busy, setBusy] = useState<"models" | "test" | null>(null);
  const [saveState, setSaveState] = useState<
    "saved" | "pending" | "saving" | "error"
  >("saved");
  const [saveError, setSaveError] = useState("");
  const [closing, setClosing] = useState(false);
  const pending = useRef({
    settings: snapshot.settings,
    key: null as string | null,
    revision: 0,
  });
  const savedRevision = useRef(0);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const saveTask = useRef<Promise<boolean> | null>(null);

  // Serialize writes and read the latest edit after each response. An older save
  // must never replace a newer draft or leave it unsaved when the dialog closes.
  const flush = useCallback((): Promise<boolean> => {
    clearTimeout(saveTimer.current);
    if (saveTask.current) return saveTask.current;
    const persist = async () => {
      while (savedRevision.current !== pending.current.revision) {
        const edit = pending.current;
        setSaveState("saving");
        setSaveError("");
        try {
          try {
            normalizeServerUrl(edit.settings.base_url);
          } catch {
            throw new Error("Enter a valid server address.");
          }
          const next = await api.save(edit.settings, edit.key);
          savedRevision.current = edit.revision;
          onSaved(next);
        } catch (error) {
          if (edit.revision !== pending.current.revision) continue;
          setSaveState("error");
          setSaveError(errorText(error));
          return false;
        }
      }
      setSaveState("saved");
      return true;
    };
    saveTask.current = Promise.resolve()
      .then(persist)
      .finally(() => {
        saveTask.current = null;
      });
    return saveTask.current;
  }, [onSaved]);

  function queueSave(
    settings: SettingsData,
    nextKey: string | null,
    immediate = false,
  ) {
    setDraft(settings);
    setKey(nextKey);
    pending.current = {
      settings,
      key: nextKey,
      revision: pending.current.revision + 1,
    };
    setSaveState("pending");
    setSaveError("");
    clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(
      () => {
        void flush();
      },
      immediate ? 0 : 500,
    );
  }

  useEffect(() => () => clearTimeout(saveTimer.current), []);
  const request = useRef("");
  const previousOpen = useRef(false);

  useEffect(() => {
    if (open && !previousOpen.current) {
      clearTimeout(saveTimer.current);
      pending.current = { settings: snapshot.settings, key: null, revision: 0 };
      savedRevision.current = 0;
      setSaveState("saved");
      setSaveError("");
      setClosing(false);
      setDraft(snapshot.settings);
      setKey(null);
      setNotice(null);
      setModels([]);
      setBusy(null);
      setTab("connection");
      setShowKey(false);
    }
    previousOpen.current = open;
    if (!open && request.current) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
  }, [open, snapshot.settings]);

  function update<K extends keyof SettingsData>(
    field: K,
    value: SettingsData[K],
  ) {
    const keepModelRequest = field === "model" && busy === "models";
    if (request.current && !keepModelRequest) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
    if (!keepModelRequest) setBusy(null);
    setNotice(null);
    let nextKey = pending.current.key;
    if (field === "base_url") {
      setModels([]);
      try {
        if (
          normalizeServerUrl(String(value)).origin !==
          normalizeServerUrl(pending.current.settings.base_url).origin
        )
          nextKey = "";
      } catch {
        nextKey = "";
      }
    }
    queueSave(
      { ...pending.current.settings, [field]: value },
      nextKey,
      field !== "base_url" && field !== "model",
    );
  }

  function updateKey(value: string) {
    setModels([]);
    setNotice(null);
    if (request.current) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
    setBusy(null);
    queueSave(pending.current.settings, value);
  }

  async function loadModels() {
    const id = crypto.randomUUID();
    request.current = id;
    setBusy("models");
    setNotice(null);
    try {
      const result = await api.models(draft, key, id);
      if (request.current !== id) return;
      setModels(result.models);
      if (!pending.current.settings.model && result.models.length === 1)
        queueSave(
          { ...pending.current.settings, model: result.models[0] },
          pending.current.key,
          true,
        );
      setNotice({
        kind: "info",
        text: result.models.length
          ? t(
              "{count} models found{partial}. Choose one with vision support.",
              {
                count: result.models.length,
                partial: result.partial ? t(" (partial list)") : "",
              },
            )
          : t("No models found. You can still enter an exact model ID."),
      });
    } catch (error) {
      if (request.current === id)
        setNotice({ kind: "error", text: errorText(error) });
    } finally {
      if (request.current === id) {
        request.current = "";
        setBusy(null);
      }
    }
  }

  async function test() {
    const id = crypto.randomUUID();
    request.current = id;
    setBusy("test");
    setNotice(null);
    try {
      const text = await api.test(draft, key, id);
      if (request.current === id) setNotice({ kind: "success", text });
    } catch (error) {
      if (request.current === id)
        setNotice({ kind: "error", text: errorText(error) });
    } finally {
      if (request.current === id) {
        request.current = "";
        setBusy(null);
      }
    }
  }

  async function close() {
    if (closing) return;
    setClosing(true);
    if (request.current) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
    setBusy(null);
    const saved = await flush();
    if (saved) {
      if (request.current) {
        void api.cancelRequest(request.current);
        request.current = "";
      }
      setKey(null);
      pending.current.key = null;
      onOpenChange(false);
    }
    setClosing(false);
  }

  function discard() {
    if (request.current) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
    clearTimeout(saveTimer.current);
    pending.current = {
      settings: snapshot.settings,
      key: null,
      revision: savedRevision.current,
    };
    setDraft(snapshot.settings);
    setKey(null);
    setSaveError("");
    setSaveState("saved");
    onOpenChange(false);
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (value) onOpenChange(true);
        else void close();
      }}
    >
      <DialogContent
        title={t("Settings")}
        description={t("Make it yours. Changes save automatically.")}
        closeLabel="Close settings"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          returnFocus();
        }}
      >
        <div
          className="settings-tabs"
          role="tablist"
          aria-label={t("Settings sections")}
        >
          <button
            type="button"
            role="tab"
            aria-selected={tab === "connection"}
            aria-controls="connection-panel"
            id="connection-tab"
            onClick={() => setTab("connection")}
          >
            <Wifi size={16} />
            {t("Connection")}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "preferences"}
            aria-controls="preferences-panel"
            id="preferences-tab"
            onClick={() => setTab("preferences")}
          >
            <SlidersHorizontal size={16} />
            {t("Preferences")}
          </button>
        </div>
        <div className="settings-body" inert={closing}>
          {tab === "connection" ? (
            <section
              role="tabpanel"
              id="connection-panel"
              aria-labelledby="connection-tab"
            >
              <div className="field">
                <div className="label-row">
                  <label htmlFor="server-url">{t("Server URL")}</label>
                </div>
                <input
                  id="server-url"
                  type="text"
                  spellCheck={false}
                  autoComplete="off"
                  value={draft.base_url}
                  onChange={(e) => update("base_url", e.target.value)}
                  placeholder={t("localhost:11434 or https://your-server/v1")}
                />
              </div>
              <div className="field">
                <label htmlFor="api-key">
                  {t("API key")}
                  <span className="optional">{t("optional")}</span>
                </label>
                <div className="password-input">
                  <input
                    id="api-key"
                    type={showKey ? "text" : "password"}
                    autoComplete="off"
                    spellCheck={false}
                    value={key ?? ""}
                    onChange={(e) => updateKey(e.target.value)}
                    placeholder={
                      snapshot.has_api_key && key === null
                        ? t("Saved securely on this Windows account")
                        : t("Only if your server requires one")
                    }
                  />
                  <button
                    type="button"
                    aria-label={showKey ? t("Hide API key") : t("Show API key")}
                    onClick={() => setShowKey(!showKey)}
                  >
                    {showKey ? <EyeOff size={17} /> : <Eye size={17} />}
                  </button>
                </div>
                {snapshot.has_api_key && key === null && (
                  <button
                    type="button"
                    className="text-button remove-key"
                    onClick={() => updateKey("")}
                  >
                    {t("Remove saved key")}
                  </button>
                )}
              </div>
              <div className="field">
                <div className="label-row">
                  <label htmlFor="model-id">{t("Vision model")}</label>
                  <button
                    type="button"
                    className="text-button"
                    disabled={!!busy}
                    onClick={() => void loadModels()}
                  >
                    <RefreshCw
                      size={13}
                      className={busy === "models" ? "spin" : ""}
                    />
                    {t("Load models")}
                  </button>
                </div>
                <ModelCombobox
                  value={draft.model}
                  models={models}
                  loading={busy === "models"}
                  onValueChange={(value) => update("model", value)}
                  onRequestModels={() => {
                    if (!busy && !models.length && draft.base_url.trim())
                      void loadModels();
                  }}
                />
                <p className="field-hint" id="model-hint">
                  {t(
                    "Choose a model that can understand images. Exact model IDs also work.",
                  )}
                </p>
              </div>
              <div className="test-row">
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!!busy || !draft.model.trim()}
                  onClick={() => void test()}
                >
                  {busy === "test" ? (
                    <LoaderCircle size={15} className="spin" />
                  ) : (
                    <Unplug size={15} />
                  )}
                  {t("Test connection")}
                </Button>
                <span>
                  {t("Uses a test image.")}
                  <br />
                  {t("Your desktop stays private during this check.")}
                </span>
              </div>
            </section>
          ) : (
            <section
              role="tabpanel"
              id="preferences-panel"
              aria-labelledby="preferences-tab"
            >
              <div className="preference-row">
                <div>
                  <label htmlFor="language" id="language-label">
                    {t("Language")}
                  </label>
                  <p id="language-hint">
                    {t("Use your desktop language or choose your own.")}
                  </p>
                </div>
                <Select
                  id="language"
                  value={draft.language}
                  onValueChange={(value) =>
                    update("language", value as SettingsData["language"])
                  }
                  options={[
                    {
                      value: "system",
                      label: t("Automatic"),
                      description: t("Follows your desktop language"),
                    },
                    { value: "de", label: t("Deutsch") },
                    { value: "en", label: t("English") },
                  ]}
                />
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="theme" id="theme-label">
                    {t("Appearance")}
                  </label>
                  <p id="theme-hint">{t("Match your workspace.")}</p>
                </div>
                <Select
                  id="theme"
                  value={draft.theme}
                  onValueChange={(value) =>
                    update("theme", value as SettingsData["theme"])
                  }
                  options={[
                    {
                      value: "system",
                      label: t("System"),
                      description: t("Follows your desktop appearance"),
                    },
                    { value: "light", label: t("Light") },
                    { value: "dark", label: t("Dark") },
                  ]}
                />
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="detail" id="detail-label">
                    {t("Screen detail")}
                  </label>
                  <p id="detail-hint">
                    {t("More detail sends larger images.")}
                  </p>
                </div>
                <Select
                  id="detail"
                  value={String(draft.screenshot_max_edge)}
                  onValueChange={(value) =>
                    update("screenshot_max_edge", Number(value))
                  }
                  options={[
                    {
                      value: "960",
                      label: t("Fast"),
                      description: t("Smaller images, faster responses"),
                    },
                    {
                      value: "1280",
                      label: t("Balanced"),
                      description: t("Best for most tasks"),
                    },
                    {
                      value: "1920",
                      label: t("Detailed"),
                      description: t("Sharper text and small controls"),
                    },
                  ]}
                />
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="timeout" id="timeout-label">
                    {t("Response timeout")}
                  </label>
                  <p id="timeout-hint">
                    {t("Time allowed for each model response.")}
                  </p>
                </div>
                <Select
                  id="timeout"
                  value={String(draft.request_timeout_seconds)}
                  onValueChange={(value) =>
                    update("request_timeout_seconds", Number(value))
                  }
                  options={[
                    ...new Set([
                      30,
                      60,
                      120,
                      180,
                      300,
                      draft.request_timeout_seconds,
                    ]),
                  ]
                    .sort((a, b) => a - b)
                    .map((value) => ({
                      value: String(value),
                      label: t("{count} seconds", { count: value }),
                    }))}
                />
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="max-steps" id="max-steps-label">
                    {t("Task limit")}
                  </label>
                  <p id="max-steps-hint">
                    {t("Stops automatically after this many steps.")}
                  </p>
                </div>
                <Select
                  id="max-steps"
                  value={String(draft.max_steps)}
                  onValueChange={(value) => update("max_steps", Number(value))}
                  options={[...new Set([10, 25, 50, 100, draft.max_steps])]
                    .sort((a, b) => a - b)
                    .map((value) => ({
                      value: String(value),
                      label: t(value === 1 ? "{count} step" : "{count} steps", {
                        count: value,
                      }),
                    }))}
                />
              </div>
            </section>
          )}
          {notice && tab === "connection" && (
            <div
              className={`notice notice-${notice.kind}`}
              role={notice.kind === "error" ? "alert" : "status"}
            >
              {t(notice.text)}
              {busy === "test" && (
                <button
                  onClick={() => {
                    void api.cancelRequest(request.current);
                    request.current = "";
                    setBusy(null);
                  }}
                >
                  {t("Cancel")}
                </button>
              )}
            </div>
          )}
          {busy === "test" && tab === "connection" && (
            <div className="notice notice-info" role="status">
              {t("Checking image understanding…")}{" "}
              <button
                className="text-button"
                onClick={() => {
                  void api.cancelRequest(request.current);
                  request.current = "";
                  setBusy(null);
                }}
              >
                {t("Cancel")}
              </button>
            </div>
          )}
          {saveError && (
            <div className="notice notice-error" role="alert">
              <strong>{t("Changes not saved.")}</strong>
              <p>{t(saveError)}</p>
              <div className="settings-error-actions">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => void flush()}
                >
                  {t("Try again")}
                </Button>
                <Button variant="ghost" size="sm" onClick={discard}>
                  {t("Discard unsaved changes")}
                </Button>
              </div>
            </div>
          )}
          {snapshot.config_error && (
            <div className="notice notice-error" role="alert">
              {t(snapshot.config_error)}
            </div>
          )}
        </div>
        <div className="settings-footer">
          <div className="config-location">
            <FolderLock size={15} />
            <span title={snapshot.config_path}>
              {snapshot.platform === "preview"
                ? t("Preview settings")
                : t("Saved beside the app in config.cfg")}
            </span>
          </div>
          <span
            className="settings-save-status"
            role="status"
            aria-live="polite"
          >
            {saveState === "saving" ? (
              <LoaderCircle className="spin" size={15} />
            ) : saveState === "saved" ? (
              <Check size={15} />
            ) : null}
            {saveState === "saving"
              ? t("Saving…")
              : saveState === "pending"
                ? t("Changes save automatically")
                : saveState === "error"
                  ? t("Changes not saved.")
                  : t("All changes saved")}
          </span>
        </div>
      </DialogContent>
    </Dialog>
  );
}
