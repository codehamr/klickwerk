import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowRight,
  Check,
  CheckCheck,
  ChevronDown,
  CircleHelp,
  Command,
  FileText,
  Globe2,
  LoaderCircle,
  Mic,
  Monitor,
  Plus,
  Settings2,
  ShieldCheck,
  Sparkles,
  Square,
  WandSparkles,
  X,
} from "lucide-react";
import { api, native } from "./lib/bridge";
import { activePhases, type Snapshot } from "./lib/types";
import { errorText, formatTime } from "./lib/utils";
import { Button } from "./components/ui/button";
import { Tooltip, TooltipProvider } from "./components/ui/tooltip";
import { Shortcut } from "./components/shortcut";
import { SettingsDialog } from "./components/settings";

const suggestions = [
  {
    icon: FileText,
    title: "Start something",
    detail: "Turn an idea into a document",
    prompt:
      "Open a text editor and draft a short, friendly welcome note for a new team member. Leave it open for me to review.",
  },
  {
    icon: Globe2,
    title: "Find something",
    detail: "Let me handle the clicking",
    prompt:
      "Open my browser and search for easy vegetarian dinner ideas. Leave the results open for me.",
  },
  {
    icon: WandSparkles,
    title: "Make it easier",
    detail: "Take care of a small task",
    prompt: "Open Calculator and work out 18% of 245. Show me the result.",
  },
];

export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [fatal, setFatal] = useState("");
  const [task, setTask] = useState("");
  const [reply, setReply] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [notice, setNotice] = useState("");
  const [mic, setMic] = useState(false);
  const [details, setDetails] = useState(false);
  const [dismissedRun, setDismissedRun] = useState(-1);
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const replyRef = useRef<HTMLInputElement>(null);
  const originalTask = useRef("");
  const receivedUpdate = useRef(false);
  const run = snapshot?.run;
  const active = !!run && activePhases.includes(run.phase);
  const showRun = !!run && run.phase !== "idle" && run.id !== dismissedRun;
  const focusPrompt = useCallback(() => {
    requestAnimationFrame(() => promptRef.current?.focus());
  }, []);

  useEffect(() => {
    let alive = true;
    const stateSub = api.onState((next) => {
      if (!alive) return;
      receivedUpdate.current = true;
      setSnapshot(next);
      setStarting(false);
      if (!activePhases.includes(next.run.phase)) setStopping(false);
    });
    const speechSub = api.onSpeech((update) => {
      if (!alive) return;
      if (update.error) setNotice(update.error);
      if (update.text)
        setTask(
          `${originalTask.current}${originalTask.current ? " " : ""}${update.text}`,
        );
      if (update.finished) {
        setMic(false);
        focusPrompt();
      }
    });
    void api
      .bootstrap()
      .then((next) => {
        if (alive && !receivedUpdate.current) setSnapshot(next);
      })
      .catch((error) => {
        if (alive) setFatal(errorText(error));
      });
    return () => {
      alive = false;
      void stateSub.then((unlisten) => unlisten());
      void speechSub.then((unlisten) => unlisten());
    };
  }, [focusPrompt]);

  useEffect(() => {
    if (!active) return;
    void api.heartbeat();
    const timer = setInterval(() => {
      void api.heartbeat().catch(() => undefined);
    }, 250);
    return () => clearInterval(timer);
  }, [active]);

  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const theme = snapshot?.settings.theme ?? "system";
      document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
      document.documentElement.dataset.reduceMotion = String(
        snapshot?.settings.reduce_motion ?? false,
      );
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [snapshot?.settings.theme, snapshot?.settings.reduce_motion]);

  useEffect(() => {
    if (run?.phase === "waiting") replyRef.current?.focus();
  }, [run?.phase]);

  const stop = useCallback(async () => {
    setStopping(true);
    try {
      await api.stop();
    } catch (error) {
      setNotice(errorText(error));
      setStopping(false);
    }
  }, []);

  useEffect(() => {
    if (native || !active) return;
    function hotkey(event: KeyboardEvent) {
      if (event.ctrlKey && event.altKey && event.key === "F8") {
        event.preventDefault();
        void stop();
      }
    }
    window.addEventListener("keydown", hotkey);
    return () => window.removeEventListener("keydown", hotkey);
  }, [active, stop]);

  async function start(answer?: string) {
    if (!snapshot || starting || active || mic || !task.trim()) return;
    if (!snapshot.settings.model || snapshot.config_error) {
      setSettingsOpen(true);
      return;
    }
    setStarting(true);
    setNotice("");
    setDetails(false);
    try {
      await api.heartbeat();
      await api.start(task.trim(), answer);
      setReply("");
    } catch (error) {
      setNotice(errorText(error));
    } finally {
      setStarting(false);
    }
  }

  async function toggleMic() {
    setNotice("");
    if (mic) {
      await api.speechStop();
      setMic(false);
      focusPrompt();
      return;
    }
    originalTask.current = task;
    setMic(true);
    try {
      await api.speechStart();
    } catch (error) {
      setMic(false);
      setNotice(errorText(error));
    }
  }

  async function stopTest() {
    setNotice("");
    setStarting(true);
    try {
      await api.heartbeat();
      await api.stopTest();
    } catch (error) {
      setNotice(errorText(error));
    } finally {
      setStarting(false);
    }
  }

  if (!snapshot)
    return (
      <div className="boot-screen">
        <div className="brand-mark">
          <Command />
        </div>
        {fatal ? (
          <div role="alert">
            <h1>Couldn't open klickwerk</h1>
            <p>{fatal}</p>
            <Button onClick={() => location.reload()}>Try again</Button>
          </div>
        ) : (
          <>
            <LoaderCircle className="spin" size={20} />
            <p>Making a little room for you…</p>
          </>
        )}
      </div>
    );

  return (
    <TooltipProvider>
      <div className="app-shell">
        {!native && (
          <div
            className="preview-banner"
            role="region"
            aria-label="Browser preview"
          >
            <Monitor size={12} />
            <span>Browser preview · all desktop actions are simulated</span>
          </div>
        )}
        <header className="app-header">
          <a
            className="brand"
            href="#"
            onClick={(event) => {
              event.preventDefault();
              if (!active) {
                setDismissedRun(run?.id ?? -1);
                focusPrompt();
              }
            }}
            aria-label="klickwerk home"
          >
            <span className="brand-mark">
              <Command size={22} strokeWidth={2.3} />
            </span>
            <span>
              klickwerk<span className="brand-period">.</span>
            </span>
          </a>
          <div className="header-actions">
            <span className="desktop-label">
              <Monitor size={14} />
              Your desktop companion
            </span>
            <Tooltip label="Settings">
              <Button
                variant="ghost"
                size="icon"
                aria-label="Open settings"
                disabled={active || starting || mic}
                onClick={() => setSettingsOpen(true)}
              >
                <Settings2 size={20} />
              </Button>
            </Tooltip>
          </div>
        </header>

        <main className={`main-content${showRun ? " has-run" : ""}`}>
          <section className="welcome" aria-labelledby="welcome-title">
            <div className="eyebrow">
              <span className="tiny-star">✦</span>A LITTLE HELP. A LOT LESS
              BUSYWORK.
            </div>
            <h1 id="welcome-title">
              What can I take
              <br />
              <span>off your hands?</span>
            </h1>
            <p>Tell me what you need. I'll handle the clicks.</p>
          </section>

          <section
            className={`composer${active ? " composer-active" : ""}${mic ? " composer-listening" : ""}`}
            aria-label="Your task"
          >
            <label className="sr-only" htmlFor="task">
              What would you like me to do?
            </label>
            <textarea
              ref={promptRef}
              id="task"
              autoFocus
              placeholder="Describe a task, just as you'd ask a person…"
              value={task}
              maxLength={8192}
              disabled={
                active ||
                starting ||
                mic ||
                (showRun && run?.phase === "waiting")
              }
              onChange={(event) => setTask(event.target.value)}
              onKeyDown={(event) => {
                if (
                  event.ctrlKey &&
                  event.key === "Enter" &&
                  !event.nativeEvent.isComposing
                ) {
                  event.preventDefault();
                  void start();
                }
              }}
            />
            {mic && (
              <div className="listening-label" role="status">
                <span className="recording-dot" />
                Listening… Speak naturally, then stop the microphone.
              </div>
            )}
            <div className="composer-toolbar">
              <div className="composer-tools">
                <Tooltip label={mic ? "Finish dictation" : "Dictate your task"}>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={mic ? "Stop microphone" : "Dictate your task"}
                    aria-pressed={mic}
                    disabled={active || starting}
                    onClick={() =>
                      void toggleMic().catch((error) =>
                        setNotice(errorText(error)),
                      )
                    }
                  >
                    <Mic size={19} />
                  </Button>
                </Tooltip>
                <span className="toolbar-divider" />
                <button
                  className="model-chip"
                  disabled={active || starting || mic}
                  onClick={() => setSettingsOpen(true)}
                >
                  <span
                    className={`status-dot${snapshot.settings.model ? " configured" : ""}`}
                  />
                  <span>{snapshot.settings.model || "Connect a model"}</span>
                  <ChevronDown size={13} />
                </button>
              </div>
              {active ? (
                <Button
                  variant="destructive"
                  disabled={stopping}
                  onClick={() => void stop()}
                >
                  <Square size={13} fill="currentColor" />
                  {stopping ? "Stopping…" : "Stop task"}
                </Button>
              ) : (
                <Tooltip label="Start task · Ctrl + Enter">
                  <Button
                    className="start-button"
                    disabled={!task.trim() || starting || mic}
                    onClick={() => void start()}
                  >
                    {starting ? (
                      <LoaderCircle className="spin" size={16} />
                    ) : (
                      <>
                        Let's do it
                        <ArrowRight size={17} />
                      </>
                    )}
                  </Button>
                </Tooltip>
              )}
            </div>
          </section>
          <div className="composer-caption">
            <span>
              <ShieldCheck size={13} />
              You stay in control. Always.
            </span>
            <span className="enter-hint">
              Ctrl <span>+</span> Enter to start
            </span>
          </div>

          {notice && (
            <div className="inline-notice" role="alert">
              <CircleHelp size={18} />
              <span>{notice}</span>
              <button
                aria-label="Dismiss message"
                onClick={() => setNotice("")}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {snapshot.config_error && (
            <div className="inline-notice" role="alert">
              <CircleHelp size={18} />
              <span>{snapshot.config_error}</span>
              <button
                className="text-button"
                onClick={() => setSettingsOpen(true)}
              >
                Open settings
              </button>
            </div>
          )}

          {showRun && run ? (
            <section
              className={`run-card run-${run.phase}`}
              aria-label="Task progress"
            >
              <div className="run-heading">
                <div className={`run-symbol ${active ? "working" : ""}`}>
                  {active ? (
                    <LoaderCircle className="spin" size={21} />
                  ) : run.phase === "done" ? (
                    <CheckCheck size={22} />
                  ) : run.phase === "waiting" ? (
                    <CircleHelp size={21} />
                  ) : (
                    <Square size={16} />
                  )}
                </div>
                <div className="run-heading-text">
                  <h2>
                    {run.phase === "checking"
                      ? "Getting ready"
                      : run.phase === "countdown"
                        ? "Your desktop is next"
                        : run.phase === "running"
                          ? "On it. You can take a breather."
                          : run.phase === "waiting"
                            ? "A quick question"
                            : run.phase === "done"
                              ? "All taken care of"
                              : run.phase === "stopped"
                                ? "Back in your hands"
                                : "Let’s get this sorted"}
                  </h2>
                  <p role="status" aria-live="polite">
                    {run.message}
                  </p>
                </div>
                <span className="elapsed">{formatTime(run.elapsed_ms)}</span>
              </div>
              {active && (
                <div className="running-safety">
                  <span>
                    <span className="recording-dot" />
                    Stop from any app
                  </span>
                  <Shortcut compact />
                  {!native && (
                    <span className="preview-hotkey-hint">
                      In this preview tab
                    </span>
                  )}
                </div>
              )}
              {run.phase === "waiting" && (
                <div className="question">
                  <p>{run.question}</p>
                  <form
                    onSubmit={(event) => {
                      event.preventDefault();
                      if (reply.trim()) void start(reply.trim());
                    }}
                  >
                    <input
                      ref={replyRef}
                      aria-label="Your answer"
                      placeholder="Your answer…"
                      value={reply}
                      maxLength={4096}
                      onChange={(e) => setReply(e.target.value)}
                    />
                    <Button type="submit" disabled={!reply.trim() || starting}>
                      Continue
                      <ArrowRight size={15} />
                    </Button>
                  </form>
                  <p className="field-hint">
                    Control is paused. Continuing starts a fresh countdown.
                  </p>
                </div>
              )}
              {run.result && <p className="run-result">{run.result}</p>}
              {!!run.steps.length && (
                <>
                  <button
                    className="steps-toggle"
                    aria-expanded={details}
                    onClick={() => setDetails(!details)}
                  >
                    <Check size={14} />
                    {run.steps.length}{" "}
                    {run.steps.length === 1 ? "step" : "steps"}
                    <ChevronDown
                      size={14}
                      className={details ? "rotated" : ""}
                    />
                  </button>
                  {details && (
                    <ol className="step-list">
                      {run.steps.map((step) => (
                        <li key={step.id}>
                          <span>
                            <Check size={12} />
                          </span>
                          {step.description}
                        </li>
                      ))}
                    </ol>
                  )}
                </>
              )}
              {!active && (
                <div className="run-actions">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      setTask("");
                      setNotice("");
                      setDismissedRun(run.id);
                      focusPrompt();
                    }}
                  >
                    <Plus size={15} />
                    New task
                  </Button>
                  {run.phase === "error" && (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => setSettingsOpen(true)}
                    >
                      Check connection
                    </Button>
                  )}
                </div>
              )}
            </section>
          ) : (
            <section className="suggestions" aria-labelledby="suggestion-title">
              <div className="suggestions-heading">
                <span id="suggestion-title">A few things to try</span>
                <ArrowDown size={13} />
              </div>
              <div className="suggestion-grid">
                {suggestions.map(({ icon: Icon, title, detail, prompt }) => (
                  <button
                    className="suggestion"
                    key={title}
                    onClick={() => {
                      setTask(prompt);
                      focusPrompt();
                    }}
                  >
                    <span className="suggestion-icon">
                      <Icon size={19} strokeWidth={1.7} />
                    </span>
                    <strong>
                      {title}
                      <ArrowRight size={14} />
                    </strong>
                    <span>{detail}</span>
                  </button>
                ))}
              </div>
            </section>
          )}
        </main>

        <footer className="app-footer">
          <div className="footer-note">
            <ShieldCheck size={17} />
            <span>
              Need to step in? <strong>Stop instantly.</strong>
            </span>
            <Shortcut compact />
          </div>
          <span className="footer-signature">
            <Sparkles size={13} />
            Less clicking. More living.
          </span>
        </footer>
        <SettingsDialog
          snapshot={snapshot}
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
          onSaved={(next) => {
            setSnapshot(next);
            setNotice("");
          }}
          onStopTest={() => void stopTest()}
          returnFocus={focusPrompt}
        />
      </div>
    </TooltipProvider>
  );
}
