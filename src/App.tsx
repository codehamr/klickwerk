import { setLanguage, useI18n } from "./lib/i18n";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  BookOpen,
  Check,
  Download,
  Pause,
  CircleAlert,
  Trash2,
  ChevronDown,
  CircleHelp,
  Command,
  LoaderCircle,
  Mic,
  Monitor,
  MousePointer2,
  Plus,
  Settings2,
  ShieldCheck,
  Sparkles,
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
import { taskSuggestions } from "./lib/suggestions";

export function App() {
  const { t, language } = useI18n();
  const suggestions = taskSuggestions(language);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [fatal, setFatal] = useState("");
  const [task, setTask] = useState("");
  const [refinement, setRefinement] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [starting, setStarting] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [notice, setNotice] = useState("");
  const [savedNotice, setSavedNotice] = useState("");
  const [mic, setMic] = useState(false);
  const [dismissedRun, setDismissedRun] = useState(-1);
  const [completedDetails, setCompletedDetails] = useState(false);
  const [deletingWorkflow, setDeletingWorkflow] = useState<string | null>(null);
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
  const resumedRun = useRef<number | null>(null);
  const run = snapshot?.run;
  const recovering = run?.phase === "recovering";
  const pendingResume = snapshot?.pending_resume_run_id;
  const active = !!run && activePhases.includes(run.phase);
  const current = !!run && run.phase !== "idle" && run.id !== dismissedRun;
  const refining =
    current &&
    !!run &&
    ["stopped", "waiting", "error", "done"].includes(run.phase);
  const completed = current && run?.phase === "done";
  const value = refining ? refinement : task;
  const canContinue =
    refining && !!run && ["stopped", "error"].includes(run.phase);
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
        if (alive && !receivedUpdate.current) {
          setSnapshot(next);
          setRefinement(next.restored_refinement ?? "");
        }
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
    void api.heartbeat().catch(() => undefined);
    const timer = setInterval(() => {
      void api.heartbeat().catch(() => undefined);
    }, 250);
    return () => clearInterval(timer);
  }, [active]);
  useEffect(() => {
    if (
      !recovering ||
      pendingResume == null ||
      resumedRun.current === pendingResume
    )
      return;
    let cancelled = false;
    // The native process grants this one-use continuation only after Windows consent.
    // Wait for a live UI heartbeat before it starts a new monitored countdown.
    void api
      .heartbeat()
      .then(() => {
        if (
          cancelled ||
          stopPending.current ||
          resumedRun.current === pendingResume
        )
          return;
        resumedRun.current = pendingResume;
        return api.resumeAfterRestart(pendingResume);
      })
      .catch((error) => {
        if (!cancelled) setNotice(errorText(error));
      });
    return () => {
      cancelled = true;
    };
  }, [recovering, pendingResume]);
  useEffect(() => {
    if (native || !active || recovering) return;
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
  }, [active, recovering]);
  useEffect(() => {
    if (snapshot?.settings.language && snapshot?.locale)
      setLanguage(snapshot.settings.language, snapshot.locale);
  }, [snapshot?.settings.language, snapshot?.locale]);
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const theme = snapshot?.settings.theme ?? "system";
      document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [snapshot?.settings.theme]);
  useEffect(() => {
    if (
      run?.phase &&
      ["stopped", "waiting", "error", "done"].includes(run.phase)
    ) {
      focusPrompt();
      document
        .querySelector(".composer")
        ?.scrollIntoView({ block: "center", behavior: "smooth" });
    }
  }, [run?.phase, run?.id, focusPrompt]);

  function clearComposer() {
    setDismissedRun(-1);
    setTask("");
    setRefinement("");
    setNotice("");
    setSavedNotice("");
    setSelectedWorkflow(null);
    setCompletedDetails(false);
    setDeletingWorkflow(null);
    focusPrompt();
  }
  async function newTask() {
    if (active || starting || mic) return;
    setStarting(true);
    try {
      await api.reset();
      clearComposer();
    } catch (error) {
      setNotice(errorText(error));
    } finally {
      setStarting(false);
    }
  }
  async function useWorkflow(workflow: Workflow) {
    await api.reset();
    clearComposer();
    setTask(workflow.prompt);
    setSelectedWorkflow({ id: workflow.id, name: workflow.name });
    focusPrompt();
  }
  async function deleteWorkflow(id: string) {
    await api.deleteWorkflow(id);
    clearComposer();
    setSavedNotice(t("Workflow deleted. A fresh session is ready."));
  }
  function prepareWorkflow() {
    if (!run) return;
    setWorkflowSource({
      run,
      correction: refinement.trim(),
      name: snapshot?.workflows.find((w) => w.id === run.workflow_id)?.name,
    });
  }
  async function exportHistory() {
    if (!run || active || starting || mic || exporting) return;
    setExporting(true);
    setNotice("");
    try {
      const path = await api.exportSession(run.id, refinement);
      if (path)
        setSavedNotice(
          t("History exported to {name}.", {
            name: path.split(/[\\/]/).pop() ?? path,
          }),
        );
    } catch (error) {
      setNotice(errorText(error));
    } finally {
      setExporting(false);
    }
  }
  async function restartAsAdministrator() {
    if (!run || active || starting || mic || exporting) return;
    setStarting(true);
    setNotice("");
    try {
      await api.restartAsAdministrator(run.id, refinement);
    } catch (error) {
      setNotice(errorText(error));
    } finally {
      setStarting(false);
    }
  }
  async function start() {
    if (
      !snapshot ||
      starting ||
      active ||
      mic ||
      (!value.trim() && !canContinue)
    )
      return;
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

  async function stop() {
    stopPending.current = true;
    try {
      await api.stop();
    } catch (error) {
      stopPending.current = false;
      setNotice(errorText(error));
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
            <h1>{t("Couldn't open klickwerk")}</h1>
            <p>{fatal}</p>
            <Button onClick={() => location.reload()}>{t("Try again")}</Button>
          </div>
        ) : (
          <>
            <LoaderCircle className="spin" size={20} />
            <p>{t("Getting ready…")}</p>
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
            aria-label={t("Browser preview")}
          >
            <Monitor size={12} />
            <span>
              {t("Browser preview · all desktop actions are simulated")}
            </span>
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
            aria-label={t("klickwerk home")}
          >
            <span className="brand-mark">
              <Command size={22} strokeWidth={2.3} />
            </span>
            <span>
              {t("klickwerk")}
              <span className="brand-period">.</span>
            </span>
          </a>
          <div className="header-actions">
            {active && (
              <Button variant="secondary" onClick={() => void stop()}>
                <Pause size={16} />
                {t("Stop")}
              </Button>
            )}
            <span className="desktop-label">
              {t("A little less busywork.")}
            </span>
            <Tooltip label={t("Settings")}>
              <Button
                variant="ghost"
                size="icon"
                aria-label={t("Open settings")}
                disabled={active || starting || mic}
                onClick={() => setSettingsOpen(true)}
              >
                <Settings2 size={20} />
              </Button>
            </Tooltip>
          </div>
        </header>
        <main className={`main-content${current ? " has-run" : ""}`}>
          <section className="welcome" aria-labelledby="welcome-title">
            <div className="eyebrow">
              <Sparkles size={14} />
              {t("YOUR DESKTOP. A LITTLE LIGHTER.")}
            </div>
            <h1 id="welcome-title">
              {refining ? (
                <>
                  <span>{t("Back to you.")}</span>
                </>
              ) : active ? (
                <>
                  {t("A little help,")} <span>{t("in motion.")}</span>
                </>
              ) : (
                <>
                  {t("What can I take")}
                  <br />
                  <span>{t("off your hands?")}</span>
                </>
              )}
            </h1>
            <p>
              {refining
                ? t("Review the result, refine it, or keep it for next time.")
                : recovering
                  ? t(
                      "Your task continues after Windows permission is approved.",
                    )
                  : active
                    ? t("Move your mouse or press any key to take over.")
                    : t(
                        "Describe the outcome. I’ll handle the clicks and typing.",
                      )}
            </p>
          </section>

          {current && run && (
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
                  {t("New task")}
                </Button>
              )}
            </div>
          )}
          <section
            className={`composer${active ? " composer-active" : ""}${refining ? " composer-refining" : ""}${mic ? " composer-listening" : ""}`}
            aria-label={refining ? t("Refine your task") : t("Your task")}
          >
            {refining && run && (
              <div
                className={`refinement-intro handoff handoff-${run.phase}`}
                role={completed ? "status" : "alert"}
                key={`${run.id}-${run.phase}`}
              >
                <span className="refinement-icon">
                  {completed ? (
                    <Check size={22} />
                  ) : run.phase === "error" ? (
                    <CircleAlert size={22} />
                  ) : (
                    <Pause size={22} />
                  )}
                </span>
                <div>
                  <h2>
                    {snapshot.can_restart_elevated
                      ? t("Administrator rights needed")
                      : completed
                        ? t("Done. Back to you.")
                        : run.phase === "error"
                          ? t("Couldn’t finish. Back to you.")
                          : run.phase === "waiting"
                            ? t("Your answer is needed")
                            : run.interrupted
                              ? t("You took over. The agent is paused.")
                              : t("Paused. Back to you.")}
                  </h2>
                  {!run.interrupted && (
                    <p>
                      {completed
                        ? t(run.result)
                        : run.phase === "waiting"
                          ? t(run.question)
                          : t(run.message)}
                    </p>
                  )}
                  <p className="handoff-next">
                    {snapshot.can_restart_elevated
                      ? t(
                          "Windows will ask for permission. Your task and history are kept. Click Continue after the restart.",
                        )
                      : completed
                        ? t(
                            "Refine the result below, save what you learned, or start a new task.",
                          )
                        : run.phase === "waiting"
                          ? t(
                              "Answer below to continue, or save this workflow for later.",
                            )
                          : t(
                              "Continue as is, or add a correction below. Nothing runs until you choose.",
                            )}
                  </p>
                  {snapshot.can_restart_elevated && (
                    <Button
                      variant="secondary"
                      className="restart-button"
                      disabled={starting || exporting || mic}
                      onClick={() => void restartAsAdministrator()}
                    >
                      {starting ? (
                        <LoaderCircle className="spin" size={16} />
                      ) : (
                        <ShieldCheck size={16} />
                      )}
                      {t("Restart as administrator")}
                    </Button>
                  )}
                </div>
              </div>
            )}
            {selectedWorkflow && !refining && !active && (
              <div className="selected-workflow">
                <BookOpen size={14} />
                <span>{selectedWorkflow.name}</span>
                <button
                  aria-label={t("Detach workflow")}
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
                  {recovering
                    ? t(
                        pendingResume == null
                          ? "Waiting for Windows permission"
                          : "Resuming your task",
                      )
                    : run?.phase === "countdown"
                      ? t("Starting in a moment")
                      : run?.phase === "checking"
                        ? t("Getting ready")
                        : t("Working on your desktop")}
                </h2>
                <p>{t(run?.message ?? "")}</p>
                {run?.phase === "countdown" && (
                  <div className="countdown-track" />
                )}
                <span className="active-caption">
                  {t(
                    recovering
                      ? "Use Stop to cancel continuation."
                      : "Move your mouse or press any key to interrupt.",
                  )}
                </span>
              </div>
            ) : (
              <>
                <label className="sr-only" htmlFor="task">
                  {refining
                    ? run?.phase === "waiting"
                      ? t("Your answer")
                      : t("Your refinement")
                    : t("What would you like me to do?")}
                </label>
                <textarea
                  ref={promptRef}
                  id="task"
                  autoFocus
                  placeholder={
                    refining
                      ? t("What would you like to change?")
                      : t("Describe a task, just as you’d ask a person…")
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
                    {t("Listening… Speak naturally, then stop the microphone.")}
                  </div>
                )}
                <div className="composer-toolbar">
                  <div className="composer-tools">
                    <Tooltip
                      label={
                        mic ? t("Finish dictation") : t("Dictate your task")
                      }
                    >
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={
                          mic ? t("Stop microphone") : t("Dictate your task")
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
                        {snapshot.settings.model || t("Connect a model")}
                      </span>
                      <ChevronDown size={13} />
                    </button>
                  </div>
                  <Tooltip label={t("Start task · Ctrl + Enter")}>
                    <Button
                      className="start-button"
                      disabled={
                        (!value.trim() && !canContinue) || starting || mic
                      }
                      onClick={() => void start()}
                    >
                      {starting ? (
                        <LoaderCircle className="spin" size={16} />
                      ) : (
                        <>
                          {refining
                            ? value.trim()
                              ? t("Refine & continue")
                              : t("Continue")
                            : t("Let’s do it")}
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
              {t(
                recovering
                  ? "Use Stop to cancel continuation."
                  : "Move your mouse or type to take over. Anytime.",
              )}
            </span>
            {!active && (
              <span className="enter-hint">
                {t("Ctrl + Enter to")} {refining ? t("continue") : t("start")}
              </span>
            )}
          </div>
          {notice && (
            <div className="inline-notice" role="alert">
              <CircleHelp size={18} />
              <span>{t(notice)}</span>
              <button
                aria-label={t("Dismiss message")}
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
              <span>{t(snapshot.config_error)}</span>
              <button
                className="text-button"
                onClick={() => setSettingsOpen(true)}
              >
                {t("Open settings")}
              </button>
            </div>
          )}
          {refining && run?.phase === "error" && (
            <div className="inline-notice" role="alert">
              <CircleHelp size={18} />
              <span>
                {t("Review your connection, then retry or refine the task.")}
              </span>
              <button
                className="text-button"
                onClick={() => setSettingsOpen(true)}
              >
                {t("Check connection")}
              </button>
            </div>
          )}
          {refining && run && (
            <div className="learning-row">
              <span>
                <BookOpen size={16} />
                {t("Turn this session into a better start next time.")}
              </span>
              <Button
                variant="secondary"
                size="sm"
                disabled={starting || mic}
                onClick={prepareWorkflow}
              >
                {run.workflow_id ? t("Update workflow") : t("Save as workflow")}
                <ArrowRight size={14} />
              </Button>
            </div>
          )}
          {refining && run && (
            <div className="session-history-actions">
              {run.steps.length > 0 && (
                <button
                  className="text-button history-toggle"
                  onClick={() => setCompletedDetails(!completedDetails)}
                  aria-expanded={completedDetails}
                >
                  {completedDetails ? t("Hide actions") : t("View actions")}
                </button>
              )}
              <button
                className="text-button history-export"
                title={t(
                  "Includes the full history, window details and recent screenshots.",
                )}
                disabled={starting || mic || exporting}
                onClick={() => void exportHistory()}
              >
                {exporting ? (
                  <LoaderCircle size={14} className="spin" />
                ) : (
                  <Download size={14} />
                )}
                {exporting
                  ? t("Exporting history…")
                  : t("Export history (JSON)")}
              </button>
            </div>
          )}
          {current && run && (active || completedDetails) && (
            <>
              <Timeline steps={run.steps} />
              {active && (
                <div className="run-time">
                  {formatTime(run.elapsed_ms)}
                  {t("elapsed")}
                </div>
              )}
            </>
          )}

          {!active && (
            <>
              {!!snapshot.workflows.length && (
                <section
                  className="workflow-library"
                  aria-labelledby="workflow-title"
                >
                  <div className="section-heading">
                    <h2 id="workflow-title">
                      <BookOpen size={16} />
                      {t("Your workflows")}
                    </h2>
                    <span>{t("Better with every refinement")}</span>
                  </div>
                  <div className="workflow-grid">
                    {snapshot.workflows.map((workflow) => (
                      <div className="workflow-card" key={workflow.id}>
                        <button
                          className="workflow-open"
                          disabled={starting || mic}
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
                            {t("Saved start prompt")}
                          </span>
                        </button>
                        <button
                          className="workflow-delete"
                          aria-label={t("Delete {name}", {
                            name: workflow.name,
                          })}
                          disabled={starting || mic}
                          onClick={() => setDeletingWorkflow(workflow.id)}
                        >
                          <Trash2 size={16} />
                        </button>
                        {deletingWorkflow === workflow.id && (
                          <div className="delete-confirmation" role="alert">
                            <p>
                              {t(
                                "Delete this workflow and start a fresh session?",
                              )}
                            </p>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setDeletingWorkflow(null)}
                            >
                              {t("Cancel")}
                            </Button>
                            <Button
                              className="button-danger"
                              size="sm"
                              disabled={starting || mic}
                              onClick={() => {
                                setStarting(true);
                                void deleteWorkflow(workflow.id)
                                  .catch((error) => setNotice(errorText(error)))
                                  .finally(() => setStarting(false));
                              }}
                            >
                              {t("Delete workflow")}
                            </Button>
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                </section>
              )}
              {!current && (
                <section
                  className="suggestions"
                  aria-labelledby="suggestion-title"
                >
                  <div className="suggestions-heading">
                    <span id="suggestion-title">
                      {snapshot.workflows.length
                        ? t("Or try something new")
                        : t("A few things to try")}
                    </span>
                  </div>
                  <div className="suggestion-grid">
                    {suggestions.map(
                      ({ icon: Icon, title, detail, prompt }) => (
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
                      ),
                    )}
                  </div>
                </section>
              )}
              {!current && !snapshot.workflows.length && (
                <p className="workflow-empty">
                  <BookOpen size={14} />
                  {t(
                    "A task worth repeating? Refine it as you go, then save it here.",
                  )}
                </p>
              )}
            </>
          )}
          {snapshot.workflow_error && (
            <div className="inline-notice" role="alert">
              <span>{t(snapshot.workflow_error)}</span>
            </div>
          )}
        </main>
        <footer className="app-footer">
          <span className="footer-note">
            <span className="status-dot configured" />
            {t("Your pace. Your control.")}
          </span>
          <span className="footer-signature">
            <Sparkles size={13} />
            {t("Less clicking. More living.")}
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
            onDelete={deleteWorkflow}
            onSaved={(workflow, learned) => {
              setSelectedWorkflow((current) =>
                current?.id === workflow.id
                  ? { id: workflow.id, name: workflow.name }
                  : current,
              );
              if (selectedWorkflow?.id === workflow.id && !current)
                setTask(workflow.prompt);
              setSavedNotice(
                t(
                  learned
                    ? "“{name}” learned from this session. The improved start prompt is saved."
                    : "“{name}” is saved with your corrections. Learning was unavailable.",
                  { name: workflow.name },
                ),
              );
            }}
          />
        )}
      </div>
    </TooltipProvider>
  );
}
