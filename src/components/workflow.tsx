import { useI18n } from "../lib/i18n";
import { useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  BookOpen,
  LoaderCircle,
  Save,
  Trash2,
  Download,
} from "lucide-react";
import { api, native } from "../lib/bridge";
import type { Learning, Run, Workflow, WorkflowDraft } from "../lib/types";
import { workflowSlug } from "../lib/workflows";
import { errorText } from "../lib/utils";
import { Dialog, DialogContent } from "./ui/dialog";
import { Button } from "./ui/button";

export type WorkflowSource =
  | { run: Run; correction: string; name?: string; reviewTraining?: boolean }
  | { workflowId: string }
  | { prompt: string; name?: string; imported?: boolean };
export function WorkflowDialog({
  source,
  onClose,
  onUse,
  onSaved,
  onDelete,
}: {
  source: WorkflowSource;
  onClose: () => void;
  onUse: (workflow: Workflow) => Promise<void>;
  onSaved: (workflow: Workflow, learned: boolean) => void;
  onDelete: (id: string) => Promise<void>;
}) {
  const { t } = useI18n();
  const [workflow, setWorkflow] = useState<Workflow | null>(() =>
    "run" in source
      ? {
          id: source.run.workflow_id ?? "",
          name: source.name ?? t("My workflow"),
          prompt: source.run.task,
          updated_at: 0,
        }
      : "prompt" in source
        ? {
            id: "",
            name:
              source.name ?? source.prompt.split(/\s+/).slice(0, 3).join(" "),
            prompt: source.prompt,
            updated_at: 0,
          }
        : null,
  );
  const [busy, setBusy] = useState<"learning" | "saving" | "deleting" | null>(
    null,
  );
  const [error, setError] = useState("");
  const [reviewed, setReviewed] = useState<WorkflowDraft | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportNotice, setExportNotice] = useState("");
  const [fallback, setFallback] = useState<WorkflowDraft | null>(null);
  const [retry, setRetry] = useState(0);
  const [deleting, setDeleting] = useState(false);
  const [original, setOriginal] = useState<Workflow | null>(null);
  const request = useRef("");
  const saving = useRef(false);
  useEffect(() => {
    let alive = true;
    if ("workflowId" in source) {
      void api
        .getWorkflow(source.workflowId)
        .then((value) => {
          if (alive) {
            setWorkflow(value);
            setOriginal(value);
            setError("");
          }
        })
        .catch((error) => {
          if (alive) setError(errorText(error));
        });
    }
    return () => {
      alive = false;
      if (request.current) void api.cancelRequest(request.current);
    };
  }, [source, retry]);
  useEffect(() => {
    if (!("run" in source) || !source.reviewTraining) return;
    let alive = true;
    const requestId = crypto.randomUUID();
    request.current = requestId;
    setBusy("learning");
    void api
      .prepareWorkflow(
        { name: source.name ?? t("My workflow"), prompt: source.run.task },
        requestId,
        source.run.id,
        source.correction,
      )
      .then((draft) => {
        if (!alive) return;
        if (draft.warning) {
          setFallback(draft);
          setError(draft.warning);
        } else {
          setWorkflow(draft.workflow);
          setReviewed(draft);
        }
      })
      .catch((error) => {
        if (alive) setError(errorText(error));
      })
      .finally(() => {
        if (alive) {
          request.current = "";
          setBusy(null);
        }
      });
    return () => {
      alive = false;
      request.current = "";
      void api.cancelRequest(requestId);
    };
  }, [source, t]);
  async function exportSaved() {
    if (!workflow || exporting) return;
    setExporting(true);
    setError("");
    try {
      if (await api.exportWorkflow(workflow.id))
        setExportNotice(t("Workflow exported."));
    } catch (error) {
      setError(errorText(error));
    } finally {
      setExporting(false);
    }
  }
  const edited =
    !original ||
    original.name !== workflow?.name ||
    original.prompt !== workflow?.prompt;
  function edit(field: keyof Learning, value: string) {
    setFallback(null);
    setWorkflow((current) => (current ? { ...current, [field]: value } : null));
  }
  async function save(use = false) {
    if (!workflow || saving.current) return;
    saving.current = true;
    const consolidate = !("prompt" in source) && !reviewed;
    setBusy(consolidate ? "learning" : "saving");
    setError("");
    setFallback(null);
    const requestId = crypto.randomUUID();
    request.current = requestId;
    try {
      const draft =
        reviewed ??
        (await api.prepareWorkflow(
          { name: workflow.name, prompt: workflow.prompt },
          requestId,
          "run" in source ? source.run.id : undefined,
          "run" in source ? source.correction : undefined,
          "workflowId" in source ? source.workflowId : undefined,
          consolidate,
        ));
      if (request.current !== requestId) return;
      if (draft.warning) {
        setFallback(draft);
        setError(draft.warning);
        return;
      }
      if ("run" in source && source.reviewTraining && !reviewed) {
        setWorkflow(draft.workflow);
        setReviewed(draft);
        return;
      }
      setBusy("saving");
      const saved = await api.saveWorkflow(
        draft.token,
        reviewed ? { name: workflow.name, prompt: workflow.prompt } : undefined,
      );
      if (use) await onUse(saved);
      onSaved(saved, consolidate || !!reviewed);
      onClose();
    } catch (error) {
      if (request.current === requestId) setError(errorText(error));
    } finally {
      if (request.current === requestId) request.current = "";
      saving.current = false;
      setBusy(null);
    }
  }
  function close() {
    if (busy === "saving" || busy === "deleting") return;
    if (request.current) {
      void api.cancelRequest(request.current);
      request.current = "";
    }
    onClose();
  }
  async function saveFallback() {
    if (!fallback || saving.current) return;
    saving.current = true;
    setBusy("saving");
    try {
      const saved = await api.saveWorkflow(fallback.token);
      onSaved(saved, false);
      onClose();
    } catch (error) {
      setError(errorText(error));
    } finally {
      saving.current = false;
      setBusy(null);
    }
  }
  async function remove() {
    if (!workflow) return;
    setBusy("deleting");
    setError("");
    try {
      await onDelete(workflow.id);
      onClose();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(null);
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) close();
      }}
    >
      <DialogContent
        title={"workflowId" in source ? t("Your workflow") : t("Save workflow")}
        description={t(
          reviewed
            ? "Your example is now a reusable prompt. Adjust anything before saving."
            : "One start prompt. Everything useful from this session built in.",
        )}
      >
        <div className="settings-body workflow-body" aria-busy={!!busy}>
          {(!workflow || busy === "learning" || busy === "saving") &&
            !error && (
              <div
                className="learning-progress"
                role="status"
                aria-live="polite"
              >
                <LoaderCircle className="spin" size={26} />
                <strong>
                  {busy === "learning"
                    ? t("Learning from this session…")
                    : busy === "saving"
                      ? t("Saving the improved start prompt…")
                      : t("Opening workflow…")}
                </strong>
                {busy === "learning" && (
                  <p>
                    {t(
                      "Combining useful steps, corrections and lessons from mistakes into one better starting point.",
                    )}
                  </p>
                )}
              </div>
            )}
          {workflow && !busy && (
            <>
              <div className="field">
                <label htmlFor="workflow-name">{t("Workflow name")}</label>
                <input
                  id="workflow-name"
                  maxLength={80}
                  value={workflow.name}
                  onChange={(e) => edit("name", e.target.value)}
                />
                <p className="field-hint workflow-filename">
                  {t("File")}: workflows/
                  {"workflowId" in source
                    ? source.workflowId
                    : "run" in source && source.run.workflow_id
                      ? source.run.workflow_id
                      : workflowSlug(workflow.name)}
                  .json
                </p>
              </div>
              <div className="field">
                <label htmlFor="workflow-prompt">{t("Start prompt")}</label>
                <p className="field-hint">
                  {t(
                    reviewed || "prompt" in source
                      ? "This is what the agent will use next time. Saving does not start the task."
                      : "Edit the goal and preferences. Saving consolidates everything into this prompt.",
                  )}
                </p>
                <textarea
                  id="workflow-prompt"
                  rows={8}
                  maxLength={8192}
                  value={workflow.prompt}
                  onChange={(e) => edit("prompt", e.target.value)}
                />
              </div>
              {"run" in source && source.correction && (
                <div className="notice notice-info">
                  <strong>{t("Your latest refinement is included")}</strong>
                  <p>{source.correction}</p>
                </div>
              )}
              <p className="field-hint">
                <BookOpen size={14} />{" "}
                {native
                  ? t(
                      "Only the consolidated start prompt is saved. Action history stays in this session.",
                    )
                  : t(
                      "Browser preview: learning is simulated. Only the start prompt is saved.",
                    )}
              </p>
            </>
          )}
          {"run" in source &&
            source.run.training?.stop_reason &&
            source.run.training.stop_reason !== "Demonstration recorded." && (
              <p className="notice notice-info">
                {t(source.run.training.stop_reason)}
              </p>
            )}
          {exportNotice && (
            <p className="notice notice-success" role="status">
              {exportNotice}
            </p>
          )}
          {"prompt" in source && source.imported && (
            <p className="notice notice-info">
              {t(
                "Review this imported prompt before saving. It will be added as a separate workflow.",
              )}
            </p>
          )}
          {error && (
            <div className="notice notice-error" role="alert">
              <strong>{t("Nothing saved yet.")}</strong>
              <p>{t(error)}</p>
              {fallback && (
                <>
                  <p>
                    {t(
                      "Learning is unavailable. Retry, or keep your prompt and corrections without consolidation.",
                    )}
                  </p>
                  <Button
                    variant="secondary"
                    disabled={!!busy}
                    onClick={() => void saveFallback()}
                  >
                    {t("Save current prompt")}
                  </Button>
                </>
              )}
              {!workflow && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setRetry(retry + 1)}
                >
                  {t("Try again")}
                </Button>
              )}
            </div>
          )}
          {deleting && !busy && (
            <div className="delete-confirmation" role="alert">
              <p>{t("Delete this workflow and start a fresh session?")}</p>
              <Button variant="ghost" onClick={() => setDeleting(false)}>
                {t("Cancel")}
              </Button>
              <Button className="button-danger" onClick={() => void remove()}>
                {t("Delete workflow")}
              </Button>
            </div>
          )}
        </div>
        <div className="settings-footer workflow-footer">
          {"workflowId" in source ? (
            <Button
              variant="ghost"
              aria-label={t("Delete")}
              disabled={!!busy || !workflow || deleting}
              onClick={() => setDeleting(true)}
            >
              <Trash2 size={16} />
              {t("Delete")}
            </Button>
          ) : (
            <Button
              variant="ghost"
              disabled={busy === "saving" || busy === "deleting"}
              onClick={close}
            >
              {t("Cancel")}
            </Button>
          )}
          {"workflowId" in source && (
            <Button
              variant="ghost"
              disabled={!!busy || !workflow || exporting || edited}
              onClick={() => void exportSaved()}
            >
              <Download size={16} />
              {t("Export workflow")}
            </Button>
          )}
          <span className="footer-spacer" />
          <Button
            variant={"workflowId" in source ? "secondary" : "default"}
            disabled={
              !!busy ||
              deleting ||
              !workflow?.name.trim() ||
              !workflow?.prompt.trim()
            }
            onClick={() => void save()}
          >
            {busy ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <Save size={15} />
            )}
            {busy
              ? t("Saving…")
              : reviewed || "prompt" in source
                ? t("Save workflow")
                : workflow?.id
                  ? t("Learn & update")
                  : t("Learn & save")}
          </Button>
          {"workflowId" in source && (
            <Button
              disabled={
                !!busy ||
                !workflow?.name.trim() ||
                !workflow?.prompt.trim() ||
                deleting
              }
              onClick={() => {
                if (edited) {
                  void save(true);
                  return;
                }
                if (workflow)
                  void onUse(workflow)
                    .then(onClose)
                    .catch((error) => setError(errorText(error)));
              }}
            >
              {edited ? t("Learn & use") : t("Use workflow")}
              <ArrowRight size={15} />
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
