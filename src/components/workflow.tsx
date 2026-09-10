import { useEffect, useState } from "react";
import { ArrowRight, BookOpen, LoaderCircle, Save, Trash2 } from "lucide-react";
import { api, native } from "../lib/bridge";
import type { Learning, Workflow } from "../lib/types";
import { errorText } from "../lib/utils";
import { Dialog, DialogContent } from "./ui/dialog";
import { Button } from "./ui/button";
import { Timeline } from "./timeline";

export type WorkflowSource =
  { runId: number; correction: string } | { workflowId: string };
export function WorkflowDialog({
  source,
  onClose,
  onUse,
  onSaved,
}: {
  source: WorkflowSource;
  onClose: () => void;
  onUse: (workflow: Workflow) => void;
  onSaved: (workflow: Workflow) => void;
}) {
  const [workflow, setWorkflow] = useState<Workflow | null>(null);
  const [token, setToken] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [deleting, setDeleting] = useState(false);
  useEffect(() => {
    let alive = true;
    const requestId = crypto.randomUUID();
    setError("");
    const request =
      "workflowId" in source
        ? api
            .getWorkflow(source.workflowId)
            .then((workflow) => ({ workflow, token: undefined }))
        : api.prepareWorkflow(source.runId, source.correction, requestId);
    void request
      .then((result) => {
        if (alive) {
          setWorkflow(result.workflow);
          setToken(result.token);
        }
      })
      .catch((error) => {
        if (alive) setError(errorText(error));
      });
    return () => {
      alive = false;
      void api.cancelRequest(requestId);
    };
  }, [source, retry]);
  function edit(field: keyof Learning, value: string) {
    setWorkflow((current) => (current ? { ...current, [field]: value } : null));
  }
  async function save(use = false) {
    if (!workflow) return;
    setBusy(true);
    setError("");
    try {
      const { name, prompt, memory } = workflow;
      const saved = await api.saveWorkflow(
        { name, prompt, memory },
        token,
        token ? undefined : workflow.id,
      );
      onSaved(saved);
      if (use) onUse(saved);
      onClose();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function remove() {
    if (!workflow) return;
    if (!deleting) {
      setDeleting(true);
      return;
    }
    setBusy(true);
    try {
      await api.deleteWorkflow(workflow.id);
      onClose();
    } catch (error) {
      setError(errorText(error));
      setBusy(false);
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <DialogContent
        title={"workflowId" in source ? "Your workflow" : "Keep what worked"}
        description="A better starting point, with your refinements built in."
      >
        <div className="settings-body workflow-body">
          {!workflow && !error && (
            <div className="learning-progress" role="status">
              <LoaderCircle className="spin" size={26} />
              <strong>Turning your refinements into a workflow…</strong>
              <p>
                Reviewing the actions and your corrections to prepare the next
                run.
              </p>
            </div>
          )}
          {workflow && (
            <>
              <div className="field">
                <label htmlFor="workflow-name">Workflow name</label>
                <input
                  id="workflow-name"
                  maxLength={80}
                  value={workflow.name}
                  onChange={(e) => edit("name", e.target.value)}
                  disabled={busy}
                />
              </div>
              <div className="field">
                <label htmlFor="workflow-prompt">Start prompt</label>
                <p className="field-hint">
                  Ready for a fresh run. Make it sound like you.
                </p>
                <textarea
                  id="workflow-prompt"
                  rows={6}
                  maxLength={8192}
                  value={workflow.prompt}
                  onChange={(e) => edit("prompt", e.target.value)}
                  disabled={busy}
                />
              </div>
              <details className="workflow-memory">
                <summary>
                  <BookOpen size={16} />
                  What the agent will remember
                </summary>
                <p className="field-hint">
                  Reusable instructions based on your corrections. You can edit
                  these too.
                </p>
                <label className="sr-only" htmlFor="workflow-memory">
                  Workflow instructions
                </label>
                <textarea
                  id="workflow-memory"
                  rows={6}
                  maxLength={8000}
                  value={workflow.memory}
                  onChange={(e) => edit("memory", e.target.value)}
                  disabled={busy}
                />
              </details>
              <details className="workflow-history">
                <summary>
                  Included history · {workflow.steps.length} entries
                </summary>
                <Timeline steps={workflow.steps} />
              </details>
              <p className="field-hint">
                {native
                  ? "Saved beside the app in workflows.json, including typed text and your corrections. Screenshots are not stored."
                  : "Preview workflow · saved only in this browser. Instructions are simulated."}
              </p>
            </>
          )}
          {error && (
            <div className="notice notice-error" role="alert">
              {error}
              {!workflow && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setRetry(retry + 1)}
                >
                  Try again
                </Button>
              )}
            </div>
          )}
        </div>
        <div className="settings-footer workflow-footer">
          {"workflowId" in source ? (
            <Button
              variant="ghost"
              size="icon"
              aria-label={
                deleting ? "Confirm delete workflow" : "Delete workflow"
              }
              disabled={busy || !workflow}
              onClick={() => void remove()}
            >
              <Trash2 size={16} />
            </Button>
          ) : (
            <Button variant="ghost" disabled={busy} onClick={onClose}>
              Cancel
            </Button>
          )}
          {deleting ? (
            <span className="field-hint">
              Delete permanently? Click again to confirm.
            </span>
          ) : (
            <span className="footer-spacer" />
          )}
          <Button
            variant="secondary"
            disabled={
              busy ||
              !workflow?.name.trim() ||
              !workflow?.prompt.trim() ||
              !workflow?.memory.trim()
            }
            onClick={() => void save()}
          >
            {busy ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <Save size={15} />
            )}
            {busy ? "Saving…" : "Save workflow"}
          </Button>
          {"workflowId" in source && (
            <Button
              disabled={
                busy ||
                !workflow?.name.trim() ||
                !workflow?.prompt.trim() ||
                !workflow?.memory.trim()
              }
              onClick={() => void save(true)}
            >
              Use workflow
              <ArrowRight size={15} />
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
