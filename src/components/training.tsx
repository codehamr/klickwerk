import { useState } from "react";
import { CircleDot, LoaderCircle } from "lucide-react";
import { useI18n } from "../lib/i18n";
import { api, native } from "../lib/bridge";
import { errorText } from "../lib/utils";
import { Dialog, DialogContent } from "./ui/dialog";
import { Button } from "./ui/button";

export function TrainingDialog({
  task,
  runId,
  workflowId,
  correction,
  onClose,
  onStarted,
}: {
  task: string;
  runId?: number;
  workflowId?: string;
  correction?: string;
  onClose: () => void;
  onStarted: () => void;
}) {
  const { t } = useI18n();
  const [text, setText] = useState(true);
  const [screenshots, setScreenshots] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function start() {
    setBusy(true);
    setError("");
    try {
      await api.heartbeat();
      await api.startTraining(
        task,
        { text, screenshots },
        runId,
        workflowId,
        correction,
      );
      onStarted();
      onClose();
    } catch (error) {
      setError(errorText(error));
    } finally {
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
        title={t("Show me how")}
        description={t(
          "Do the task once. Turn your example into a reusable workflow.",
        )}
      >
        <div className="settings-body training-setup">
          <p className="training-goal">{task}</p>
          <ol className="training-steps">
            <li>
              {t(
                "Start recording, then switch to the app you want to demonstrate.",
              )}
            </li>
            <li>
              {t(
                "Work as usual. Clicks, scrolling and window focus are recorded.",
              )}
            </li>
            <li>
              {t(
                "Finish with Ctrl + Shift + F9. Review the improved prompt and save.",
              )}
            </li>
          </ol>
          <label className="training-option">
            <input
              type="checkbox"
              checked={text}
              onChange={(e) => setText(e.target.checked)}
            />
            <span>
              <strong>{t("Include typed text")}</strong>
              <small>
                {t(
                  "Only recognized standard text fields. Password and unknown fields are omitted; clipboard content is never read.",
                )}
              </small>
            </span>
          </label>
          <label className="training-option">
            <input
              type="checkbox"
              checked={screenshots}
              onChange={(e) => setScreenshots(e.target.checked)}
            />
            <span>
              <strong>{t("Include occasional screenshots")}</strong>
              <small>
                {t(
                  "Up to 8 small images of the active window. Images may contain sensitive information.",
                )}
              </small>
            </span>
          </label>
          <p className="field-hint">
            {t(
              "Pause with Ctrl + Shift + F8 before sensitive steps. Recording stays in memory until you finish; analysis sends it to your configured model. Only the reviewed prompt is saved in the workflow.",
            )}
          </p>
          {!native && (
            <p className="notice notice-info">
              {t(
                "Browser preview: the demonstration is simulated. No desktop input is recorded.",
              )}
            </p>
          )}
          {error && (
            <p className="notice notice-error" role="alert">
              {t(error)}
            </p>
          )}
        </div>
        <div className="settings-footer">
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t("Cancel")}
          </Button>
          <span className="footer-spacer" />
          <Button disabled={busy} onClick={() => void start()}>
            {busy ? (
              <LoaderCircle className="spin" size={16} />
            ) : (
              <CircleDot size={16} />
            )}
            {t("Start recording")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
