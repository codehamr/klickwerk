import { useEffect, useRef, useState } from "react";
import * as Switch from "@radix-ui/react-switch";
import {
  Check,
  ChevronDown,
  Eye,
  EyeOff,
  FolderLock,
  LoaderCircle,
  Monitor,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  Unplug,
  Wifi,
} from "lucide-react";
import { api } from "../lib/bridge";
import type { Settings as SettingsData, Snapshot } from "../lib/types";
import { errorText } from "../lib/utils";
import { Button } from "./ui/button";
import { Dialog, DialogContent } from "./ui/dialog";
import { Shortcut } from "./shortcut";

interface Props {
  snapshot: Snapshot;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (snapshot: Snapshot) => void;
  onStopTest: () => void;
  returnFocus: () => void;
}

export function SettingsDialog({
  snapshot,
  open,
  onOpenChange,
  onSaved,
  onStopTest,
  returnFocus,
}: Props) {
  const [draft, setDraft] = useState<SettingsData>(snapshot.settings);
  const [key, setKey] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [tab, setTab] = useState<"connection" | "preferences">("connection");
  const [models, setModels] = useState<string[]>([]);
  const [notice, setNotice] = useState<{
    kind: "success" | "error" | "info";
    text: string;
  } | null>(null);
  const [busy, setBusy] = useState<
    "models" | "test" | "save" | "discover" | null
  >(null);
  const request = useRef("");
  const previousOpen = useRef(false);

  useEffect(() => {
    if (open && !previousOpen.current) {
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
    if (request.current) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
    setBusy(null);
    setNotice(null);
    if (field === "base_url") {
      setModels([]);
      try {
        if (new URL(String(value)).origin !== new URL(draft.base_url).origin)
          setKey("");
      } catch {
        setKey("");
      }
    }
    setDraft((current) => ({ ...current, [field]: value }));
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
      if (!draft.model && result.models.length === 1)
        setDraft((current) => ({ ...current, model: result.models[0] }));
      setNotice({
        kind: "info",
        text: result.models.length
          ? `${result.models.length} models found${result.partial ? " (partial list)" : ""}. Choose one with vision support.`
          : "No models found. You can still enter an exact model ID.",
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

  async function discover() {
    const id = crypto.randomUUID();
    request.current = id;
    setBusy("discover");
    setNotice(null);
    try {
      const found = await api.discover();
      if (request.current !== id) return;
      if (found.length) {
        setDraft((current) => ({
          ...current,
          base_url: found[0].base_url,
          model:
            found[0].models.length === 1 ? found[0].models[0] : current.model,
        }));
        setKey("");
        setModels(found[0].models);
        setNotice({
          kind: "success",
          text: `Found ${found[0].name} on this PC. ${found.length > 1 ? "Other servers are available; enter their URL to switch." : "Choose your vision model below."}`,
        });
      } else
        setNotice({
          kind: "info",
          text: "No local server found. Start Ollama or your compatible server, then try again.",
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

  async function save() {
    setBusy("save");
    setNotice(null);
    try {
      const next = await api.save(draft, key);
      onSaved(next);
      onOpenChange(false);
    } catch (error) {
      setNotice({ kind: "error", text: errorText(error) });
    } finally {
      setBusy(null);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (busy !== "save") onOpenChange(value);
      }}
    >
      <DialogContent
        title="Make yourself at home"
        description="A few settings. Then you're ready to go."
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          returnFocus();
        }}
      >
        <div
          className="settings-tabs"
          role="tablist"
          aria-label="Settings sections"
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
            Connection
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
            Preferences
          </button>
        </div>
        <div className="settings-body">
          {tab === "connection" ? (
            <section
              role="tabpanel"
              id="connection-panel"
              aria-labelledby="connection-tab"
            >
              <div className="connection-options" aria-label="Connection type">
                <button
                  type="button"
                  className={draft.provider === "local" ? "selected" : ""}
                  aria-pressed={draft.provider === "local"}
                  onClick={() => update("provider", "local")}
                >
                  <Monitor size={21} />
                  <span>
                    <strong>On this PC</strong>
                    <small>Ollama or a local server</small>
                  </span>
                  {draft.provider === "local" && <Check size={16} />}
                </button>
                <button
                  type="button"
                  className={draft.provider === "custom" ? "selected" : ""}
                  aria-pressed={draft.provider === "custom"}
                  onClick={() => update("provider", "custom")}
                >
                  <Wifi size={21} />
                  <span>
                    <strong>Custom server</strong>
                    <small>OpenAI-compatible API</small>
                  </span>
                  {draft.provider === "custom" && <Check size={16} />}
                </button>
              </div>
              <div className="field">
                <div className="label-row">
                  <label htmlFor="server-url">Server address</label>
                  {draft.provider === "local" && (
                    <button
                      type="button"
                      className="text-button"
                      disabled={!!busy}
                      onClick={() => void discover()}
                    >
                      {busy === "discover" ? (
                        <LoaderCircle className="spin" size={13} />
                      ) : (
                        <Wifi size={13} />
                      )}
                      Find automatically
                    </button>
                  )}
                </div>
                <input
                  id="server-url"
                  type="url"
                  spellCheck={false}
                  autoComplete="off"
                  value={draft.base_url}
                  onChange={(e) => update("base_url", e.target.value)}
                  placeholder="http://localhost:11434/v1"
                />
              </div>
              <div className="field">
                <div className="label-row">
                  <label htmlFor="model-id">Vision model</label>
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
                    Load models
                  </button>
                </div>
                <div className="input-with-icon">
                  <input
                    id="model-id"
                    list="available-models"
                    spellCheck={false}
                    autoComplete="off"
                    value={draft.model}
                    onChange={(e) => update("model", e.target.value)}
                    placeholder="Choose or enter a model ID"
                  />
                  <ChevronDown size={16} />
                </div>
                <datalist id="available-models">
                  {models.map((model) => (
                    <option key={model} value={model} />
                  ))}
                </datalist>
                <p className="field-hint">
                  Choose a model that can understand images. Exact model IDs
                  also work.
                </p>
              </div>
              <div className="field">
                <label htmlFor="api-key">
                  API key <span className="optional">optional</span>
                </label>
                <div className="password-input">
                  <input
                    id="api-key"
                    type={showKey ? "text" : "password"}
                    autoComplete="off"
                    spellCheck={false}
                    value={key ?? ""}
                    onChange={(e) => {
                      setKey(e.target.value);
                      setNotice(null);
                      if (request.current) {
                        void api.cancelRequest(request.current);
                        request.current = "";
                        setBusy(null);
                      }
                    }}
                    placeholder={
                      snapshot.has_api_key && key === null
                        ? "Saved securely on this Windows account"
                        : "Only if your server requires one"
                    }
                  />
                  <button
                    type="button"
                    aria-label={showKey ? "Hide API key" : "Show API key"}
                    onClick={() => setShowKey(!showKey)}
                  >
                    {showKey ? <EyeOff size={17} /> : <Eye size={17} />}
                  </button>
                </div>
                {snapshot.has_api_key && key === null && (
                  <button
                    type="button"
                    className="text-button remove-key"
                    onClick={() => setKey("")}
                  >
                    Remove saved key
                  </button>
                )}
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
                  Test connection
                </Button>
                <span>
                  Uses a test image.
                  <br />
                  Your desktop stays private during this check.
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
                  <label htmlFor="theme">Appearance</label>
                  <p>Match your workspace.</p>
                </div>
                <select
                  id="theme"
                  value={draft.theme}
                  onChange={(e) =>
                    update("theme", e.target.value as SettingsData["theme"])
                  }
                >
                  <option value="system">System</option>
                  <option value="light">Light</option>
                  <option value="dark">Dark</option>
                </select>
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="motion">Reduce motion</label>
                  <p>Keep transitions quiet.</p>
                </div>
                <Switch.Root
                  id="motion"
                  className="switch"
                  checked={draft.reduce_motion}
                  onCheckedChange={(value) => update("reduce_motion", value)}
                >
                  <Switch.Thumb className="switch-thumb" />
                </Switch.Root>
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="detail">Screen detail</label>
                  <p>More detail sends larger images.</p>
                </div>
                <select
                  id="detail"
                  value={draft.screenshot_max_edge}
                  onChange={(e) =>
                    update("screenshot_max_edge", Number(e.target.value))
                  }
                >
                  <option value="960">Fast</option>
                  <option value="1280">Balanced</option>
                  <option value="1920">Detailed</option>
                </select>
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="timeout">Response timeout</label>
                  <p>Time allowed for each model response.</p>
                </div>
                <select
                  id="timeout"
                  value={draft.request_timeout_seconds}
                  onChange={(e) =>
                    update("request_timeout_seconds", Number(e.target.value))
                  }
                >
                  {[
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
                    .map((value) => (
                      <option key={value} value={value}>
                        {value} seconds
                      </option>
                    ))}
                </select>
              </div>
              <div className="preference-row">
                <div>
                  <label htmlFor="max-steps">Task limit</label>
                  <p>Stops automatically after this many steps.</p>
                </div>
                <select
                  id="max-steps"
                  value={draft.max_steps}
                  onChange={(e) => update("max_steps", Number(e.target.value))}
                >
                  {[...new Set([10, 25, 50, 100, draft.max_steps])]
                    .sort((a, b) => a - b)
                    .map((value) => (
                      <option key={value} value={value}>
                        {value} steps
                      </option>
                    ))}
                </select>
              </div>
              <div className="safety-setting">
                <ShieldCheck size={21} />
                <div>
                  <strong>Your emergency stop</strong>
                  <p>Always available while a task runs.</p>
                  <Shortcut compact />
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => {
                      onOpenChange(false);
                      onStopTest();
                    }}
                  >
                    Try a safe stop test
                  </button>
                </div>
              </div>
            </section>
          )}
          {notice && (
            <div
              className={`notice notice-${notice.kind}`}
              role={notice.kind === "error" ? "alert" : "status"}
            >
              {notice.text}
              {busy === "test" && (
                <button
                  onClick={() => {
                    void api.cancelRequest(request.current);
                    request.current = "";
                    setBusy(null);
                  }}
                >
                  Cancel
                </button>
              )}
            </div>
          )}
          {busy === "test" && (
            <div className="notice notice-info" role="status">
              Checking image understanding…{" "}
              <button
                className="text-button"
                onClick={() => {
                  void api.cancelRequest(request.current);
                  request.current = "";
                  setBusy(null);
                }}
              >
                Cancel
              </button>
            </div>
          )}
          {snapshot.config_error && (
            <div className="notice notice-error" role="alert">
              {snapshot.config_error}
            </div>
          )}
        </div>
        <div className="settings-footer">
          <div className="config-location">
            <FolderLock size={15} />
            <span title={snapshot.config_path}>
              {snapshot.platform === "preview"
                ? "Preview settings"
                : "Saved beside the app in config.cfg"}
            </span>
          </div>
          <Button disabled={!!busy} onClick={() => void save()}>
            {busy === "save" && <LoaderCircle className="spin" size={16} />}Save
            settings
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
