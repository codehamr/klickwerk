import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  BookOpen,
  Check,
  ChevronDown,
  CircleHelp,
  Command,
  FileText,
  Globe2,
  LoaderCircle,
  Mic,
  Monitor,
  MousePointer2,
  Plus,
  Settings2,
  Sparkles,
  WandSparkles,
  X,
} from "lucide-react";
import { api, native } from "./lib/bridge";
import { activePhases, type Snapshot, type Workflow } from "./lib/types";
import { errorText, formatTime } from "./lib/utils";
import { Button } from "./components/ui/button";
import { Tooltip, TooltipProvider } from "./components/ui/tooltip";
import { SettingsDialog } from "./components/settings";
import { Timeline } from "./components/timeline";
import { WorkflowDialog, type WorkflowSource } from "./components/workflow";

const suggestions = [
  {
    icon: FileText,
    title: "Draft a note",
    detail: "Get a first draft on the page",
    prompt:
      "Open a text editor and draft a short, friendly welcome note for a new team member. Leave it open for me to review.",
  },
  {
    icon: Globe2,
    title: "Find an answer",
    detail: "Let me handle the browsing",
    prompt:
      "Open my browser and search for easy vegetarian dinner ideas. Leave the results open for me.",
  },
  {
    icon: WandSparkles,
    title: "Do the small things",
    detail: "A little less busywork",
    prompt: "Open Calculator and work out 18% of 245. Show me the result.",
  },
];

export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [fatal, setFatal] = useState("");
  const [task, setTask] = useState("");
  const [refinement, setRefinement] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [starting, setStarting] = useState(false);
  const [notice, setNotice] = useState("");
  const [savedNotice, setSavedNotice] = useState("");
  const [mic, setMic] = useState(false);
  const [dismissedRun, setDismissedRun] = useState(-1);
  const [completedDetails, setCompletedDetails] = useState(false);
  const [selectedWorkflow, setSelectedWorkflow] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [workflowSource, setWorkflowSource] = useState<WorkflowSource | null>(
    null,
  );
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const originalTask = useRef("");
  const dictatingRefinement = useRef(false);
  const receivedUpdate = useRef(false);
  const stopPending = useRef(false);
  const run = snapshot?.run;
  const active = !!run && activePhases.includes(run.phase);
  const current = !!run && run.phase !== "idle" && run.id !== dismissedRun;
  const refining =
    current && !!run && ["stopped", "waiting", "error"].includes(run.phase);
  const completed = current && run?.phase === "done";
  const value = refining ? refinement : task;
  const focusPrompt = useCallback(() => {
    requestAnimationFrame(() => promptRef.current?.focus());
  }, []);

  useEffect(() => {
    let alive = true;
    const stateSub = api.onState((next) => {
      if (!alive) return;
      receivedUpdate.current = true;
      setSnapshot(next);
      if (!activePhases.includes(next.run.phase)) stopPending.current = false;
    });
    const speechSub = api.onSpeech((update) => {
      if (!alive) return;
      if (update.error) setNotice(update.error);
      if (update.text) {
        const text = `${originalTask.current}${originalTask.current ? " " : ""}${update.text}`;
        if (dictatingRefinement.current) setRefinement(text);
        else setTask(text);
      }
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
    if (native || !active) return;
    function takeover(event: Event) {
      if (!event.isTrusted || stopPending.current) return;
      if (event.type === "keydown" && (event as KeyboardEvent).repeat) return;
      stopPending.current = true;
      if (event.cancelable) event.preventDefault();
      void api.stop().catch((error) => {
        stopPending.current = false;
        setNotice(errorText(error));
      });
    }
    const events = ["pointermove", "pointerdown", "keydown", "wheel"];
    events.forEach((event) =>
      window.addEventListener(event, takeover, {
        capture: true,
        passive: false,
      }),
    );
    return () =>
      events.forEach((event) =>
        window.removeEventListener(event, takeover, true),
      );
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
    if (
      run?.phase === "stopped" ||
      run?.phase === "waiting" ||
      run?.phase === "error"
    )
      focusPrompt();
    if (run?.phase === "done") {
      setTask("");
      setSelectedWorkflow(null);
      focusPrompt();
    }
  }, [run?.phase, focusPrompt]);

  function newTask() {
    if (active || starting || mic) return;
    setDismissedRun(run?.id ?? -1);
    setTask("");
    setRefinement("");
    setNotice("");
    setSavedNotice("");
    setSelectedWorkflow(null);
    setCompletedDetails(false);
    focusPrompt();
  }
  function useWorkflow(workflow: Workflow) {
    setDismissedRun(run?.id ?? -1);
    setTask(workflow.prompt);
    setRefinement("");
    setNotice("");
    setSavedNotice("");
    setSelectedWorkflow({ id: workflow.id, name: workflow.name });
    focusPrompt();
  }
  async function start() {
    if (!snapshot || starting || active || mic || !value.trim()) return;
    if (!snapshot.settings.model || snapshot.config_error) {
      setSettingsOpen(true);
      return;
    }
    setStarting(true);
    setNotice("");
    setSavedNotice("");
    setCompletedDetails(false);
    try {
      await api.heartbeat();
      await api.start(
        refining ? run!.task : task.trim(),
        refining ? refinement.trim() : undefined,
        refining ? run!.id : undefined,
        selectedWorkflow?.id,
      );
      setRefinement("");
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
    originalTask.current = value;
    dictatingRefinement.current = refining;
    setMic(true);
    try {
      await api.speechStart();
    } catch (error) {
      setMic(false);
      setNotice(errorText(error));
    }
  }
  function changePrompt(text: string) {
    if (refining) setRefinement(text);
    else {
      setTask(text);
      if (completed) setDismissedRun(run!.id);
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
            <p>Getting ready…</p>
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
            onClick={(e) => {
              e.preventDefault();
              newTask();
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
            <span className="desktop-label">A little less busywork.</span>
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
        <main
          className={`main-content${current && !completed ? " has-run" : ""}`}
        >
          <section className="welcome" aria-labelledby="welcome-title">
            <div className="eyebrow">
              <Sparkles size={14} />
              YOUR DESKTOP. A LITTLE LIGHTER.
            </div>
            <h1 id="welcome-title">
              {refining ? (
                <>
                  Let’s get it <span>just right.</span>
                </>
              ) : active ? (
                <>
                  A little help, <span>in motion.</span>
                </>
              ) : (
                <>
                  What can I take
                  <br />
                  <span>off your hands?</span>
                </>
              )}
            </h1>
            <p>
              {refining
                ? "Your next instruction makes this workflow better."
                : active
                  ? "Move your mouse or press any key to take over."
                  : "Describe the outcome. I’ll handle the clicks and typing."}
            </p>
          </section>

          {current && !completed && run && (
            <div className="task-context">
              <span>{run.task}</span>
              {!active && (
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={starting || mic}
                  onClick={newTask}
                >
                  <Plus size={14} />
                  New task
                </Button>
              )}
            </div>
          )}
          <section
            className={`composer${active ? " composer-active" : ""}${refining ? " composer-refining" : ""}${mic ? " composer-listening" : ""}`}
            aria-label={refining ? "Refine your task" : "Your task"}
          >
            {refining && (
              <div className="refinement-intro">
                <span className="refinement-icon">
                  <WandSparkles size={18} />
                </span>
                <div>
                  <h2>
                    {run?.phase === "waiting"
                      ? "A quick question"
                      : run?.interrupted
                        ? "You took over. What should I do differently?"
                        : "What should we change?"}
                  </h2>
                  <p>
                    {run?.phase === "waiting"
                      ? run.question
                      : "Tell me what went wrong or what you want instead. I’ll continue with your correction and the actions so far."}
                  </p>
                </div>
              </div>
            )}
            {selectedWorkflow && !refining && !active && (
              <div className="selected-workflow">
                <BookOpen size={14} />
                <span>{selectedWorkflow.name}</span>
                <button
                  aria-label="Detach workflow"
                  onClick={() => setSelectedWorkflow(null)}
                >
                  <X size={13} />
                </button>
              </div>
            )}
            {active ? (
              <div className="active-message" role="status" aria-live="polite">
                <span className="activity-orbit">
                  <MousePointer2 size={25} />
                </span>
                <h2>
                  {run?.phase === "countdown"
                    ? "Starting in a moment"
                    : run?.phase === "checking"
                      ? "Getting ready"
                      : "Working on your desktop"}
                </h2>
                <p>{run?.message}</p>
                {run?.phase === "countdown" && (
                  <div className="countdown-track" />
                )}
                <span className="active-caption">
                  Move your mouse or press any key to interrupt.
                </span>
              </div>
            ) : (
              <>
                <label className="sr-only" htmlFor="task">
                  {refining
                    ? run?.phase === "waiting"
                      ? "Your answer"
                      : "Your refinement"
                    : "What would you like me to do?"}
                </label>
                <textarea
                  ref={promptRef}
                  id="task"
                  autoFocus
                  placeholder={
                    refining
                      ? "For example: use the search field at the top, then open the first result…"
                      : "Describe a task, just as you’d ask a person…"
                  }
                  value={value}
                  maxLength={refining ? 4096 : 8192}
                  disabled={starting || mic}
                  onChange={(e) => changePrompt(e.target.value)}
                  onKeyDown={(e) => {
                    if (
                      e.ctrlKey &&
                      e.key === "Enter" &&
                      !e.nativeEvent.isComposing
                    ) {
                      e.preventDefault();
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
                    <Tooltip
                      label={mic ? "Finish dictation" : "Dictate your task"}
                    >
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={
                          mic ? "Stop microphone" : "Dictate your task"
                        }
                        aria-pressed={mic}
                        disabled={starting}
                        onClick={() =>
                          void toggleMic().catch((e) => setNotice(errorText(e)))
                        }
                      >
                        <Mic size={19} />
                      </Button>
                    </Tooltip>
                    <span className="toolbar-divider" />
                    <button
                      className="model-chip"
                      disabled={starting || mic}
                      onClick={() => setSettingsOpen(true)}
                    >
                      <span
                        className={`status-dot${snapshot.settings.model ? " configured" : ""}`}
                      />
                      <span>
                        {snapshot.settings.model || "Connect a model"}
                      </span>
                      <ChevronDown size={13} />
                    </button>
                  </div>
                  <Tooltip label="Start task · Ctrl + Enter">
                    <Button
                      className="start-button"
                      disabled={!value.trim() || starting || mic}
                      onClick={() => void start()}
                    >
                      {starting ? (
                        <LoaderCircle className="spin" size={16} />
                      ) : (
                        <>
                          {refining ? "Continue" : "Let’s do it"}
                          <ArrowRight size={17} />
                        </>
                      )}
                    </Button>
                  </Tooltip>
                </div>
              </>
            )}
          </section>
          <div className="composer-caption">
            <span>
              <MousePointer2 size={13} />
              Move your mouse or type to take over. Anytime.
            </span>
            {!active && (
              <span className="enter-hint">
                Ctrl + Enter to {refining ? "continue" : "start"}
              </span>
            )}
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
          {savedNotice && (
            <div className="notice notice-success" role="status">
              {savedNotice}
            </div>
          )}
          {snapshot.config_error && (
            <div className="inline-notice" role="alert">
              <span>{snapshot.config_error}</span>
              <button
                className="text-button"
                onClick={() => setSettingsOpen(true)}
              >
                Open settings
              </button>
            </div>
          )}
          {refining && run?.phase === "error" && (
            <div className="inline-notice" role="alert">
              <CircleHelp size={18} />
              <span>{run.message}</span>
              <button
                className="text-button"
                onClick={() => setSettingsOpen(true)}
              >
                Check connection
              </button>
            </div>
          )}
          {refining && run && (
            <div className="learning-row">
              <span>
                <BookOpen size={16} />
                Keep your refinements for next time.
              </span>
              <Button
                variant="secondary"
                size="sm"
                disabled={starting || mic}
                onClick={() =>
                  setWorkflowSource({
                    runId: run.id,
                    correction: refinement.trim(),
                  })
                }
              >
                Save as workflow
                <ArrowRight size={14} />
              </Button>
            </div>
          )}
          {completed && run && (
            <div className="completion-strip" role="status">
              <Check size={18} />
              <div>
                <strong>Done. Back to you.</strong>
                <p>{run.result}</p>
              </div>
              <button
                className="text-button"
                onClick={() => setCompletedDetails(!completedDetails)}
                aria-expanded={completedDetails}
              >
                View actions
              </button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  setWorkflowSource({ runId: run.id, correction: "" })
                }
              >
                Save workflow
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Dismiss completed task"
                onClick={() => setDismissedRun(run.id)}
              >
                <X size={15} />
              </Button>
            </div>
          )}
          {current && run && (!completed || completedDetails) && (
            <>
              <Timeline steps={run.steps} />
              {active && (
                <div className="run-time">
                  {formatTime(run.elapsed_ms)} elapsed
                </div>
              )}
            </>
          )}

          {!active && !refining && (
            <>
              {!!snapshot.workflows.length && (
                <section
                  className="workflow-library"
                  aria-labelledby="workflow-title"
                >
                  <div className="section-heading">
                    <h2 id="workflow-title">
                      <BookOpen size={16} />
                      Your workflows
                    </h2>
                    <span>Better with every refinement</span>
                  </div>
                  <div className="workflow-grid">
                    {snapshot.workflows.map((workflow) => (
                      <button
                        className="workflow-card"
                        key={workflow.id}
                        onClick={() =>
                          setWorkflowSource({ workflowId: workflow.id })
                        }
                      >
                        <span className="workflow-card-top">
                          <strong>{workflow.name}</strong>
                          <ArrowRight size={16} />
                        </span>
                        <p>{workflow.prompt}</p>
                        <span className="workflow-count">
                          {workflow.corrections
                            ? `${workflow.corrections} ${workflow.corrections === 1 ? "refinement" : "refinements"} remembered`
                            : "Ready to make your own"}
                        </span>
                      </button>
                    ))}
                  </div>
                </section>
              )}
              <section
                className="suggestions"
                aria-labelledby="suggestion-title"
              >
                <div className="suggestions-heading">
                  <span id="suggestion-title">
                    {snapshot.workflows.length
                      ? "Or try something new"
                      : "A few things to try"}
                  </span>
                </div>
                <div className="suggestion-grid">
                  {suggestions.map(({ icon: Icon, title, detail, prompt }) => (
                    <button
                      className="suggestion"
                      key={title}
                      onClick={() => {
                        setTask(prompt);
                        setSelectedWorkflow(null);
                        setDismissedRun(run?.id ?? -1);
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
              {!snapshot.workflows.length && (
                <p className="workflow-empty">
                  <BookOpen size={14} />A task worth repeating? Refine it as you
                  go, then save it here.
                </p>
              )}
            </>
          )}
          {snapshot.workflow_error && (
            <div className="inline-notice" role="alert">
              <span>{snapshot.workflow_error}</span>
            </div>
          )}
        </main>
        <footer className="app-footer">
          <span className="footer-note">
            <span className="status-dot configured" />
            Your pace. Your control.
          </span>
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
          returnFocus={focusPrompt}
        />
        {workflowSource && (
          <WorkflowDialog
            source={workflowSource}
            onClose={() => {
              setWorkflowSource(null);
              focusPrompt();
            }}
            onUse={useWorkflow}
            onSaved={(workflow) =>
              setSavedNotice(`“${workflow.name}” is saved in Your workflows.`)
            }
          />
        )}
      </div>
    </TooltipProvider>
  );
}
