import { useState } from "react";
import {
  Check,
  Command,
  Download,
  LoaderCircle,
  RefreshCw,
} from "lucide-react";
import { api } from "../lib/bridge";
import { useI18n } from "../lib/i18n";
import type { UpdateStatus } from "../lib/types";
import { errorText } from "../lib/utils";
import { Button } from "./ui/button";

export function UpdateScreen({ update }: { update: UpdateStatus }) {
  const { t } = useI18n();
  const [error, setError] = useState("");
  const installing = update.phase === "installing";
  const downloading = update.phase === "downloading";
  const percent = update.total
    ? Math.min(100, Math.round((100 * update.downloaded) / update.total))
    : 0;
  return (
    <main className="boot-screen update-screen" aria-labelledby="update-title">
      <div className="brand-mark">
        <Command size={24} />
      </div>
      <h1 id="update-title">{t("Getting klickwerk ready")}</h1>
      <div className="update-card">
        <div className="update-heading" role="status">
          <LoaderCircle className="spin" size={18} />
          <strong>
            {t(
              installing
                ? "Restarting with your update…"
                : downloading
                  ? "Downloading the latest version…"
                  : "Checking for updates…",
            )}
          </strong>
        </div>
        {downloading && (
          <>
            <progress
              aria-label={t("Update download")}
              max={100}
              value={percent}
            />
            <div className="update-meta">
              <span>{update.latest}</span>
              <span>{percent}%</span>
            </div>
          </>
        )}
        <p>
          {t(
            installing
              ? "Just a moment. klickwerk will reopen automatically."
              : "A quick check on GitHub at every start. Your settings and workflows stay with you.",
          )}
        </p>
        {!installing && (
          <Button
            variant="secondary"
            onClick={() => {
              void api
                .skipUpdate()
                .catch((reason) => setError(errorText(reason)));
            }}
          >
            {t("Skip this time")}
          </Button>
        )}
        {error && <p role="alert">{t(error)}</p>}
      </div>
    </main>
  );
}

export function UpdateNotice({ update }: { update?: UpdateStatus }) {
  const { t } = useI18n();
  if (!update || update.startup) return null;
  const message =
    update.phase === "available"
      ? "An update is available. It will install the next time you open klickwerk."
      : update.phase === "updated"
        ? "klickwerk is updated. You’re ready to go."
        : update.phase === "error"
          ? update.message
          : null;
  if (!message) return null;
  return (
    <div className="update-notice" role="status">
      {update.phase === "updated" ? (
        <Check size={15} />
      ) : (
        <Download size={15} />
      )}
      <span>{t(message)}</span>
    </div>
  );
}

export function UpdateCheck({ update }: { update?: UpdateStatus }) {
  const { t } = useI18n();
  const [error, setError] = useState("");
  if (!update) return null;
  const checking = update.phase === "checking";
  const label = checking
    ? "Checking for updates…"
    : update.phase === "current" || update.phase === "updated"
      ? "Up to date"
      : update.phase === "available"
        ? "Update on next start"
        : update.phase === "skipped"
          ? "Update skipped"
          : "Check for updates";
  return (
    <span className="update-check">
      <button
        type="button"
        aria-label={t("Check for updates")}
        title={`${t("Version")}: ${update.current}${update.phase === "skipped" ? ` · ${t("We’ll check again next time you open klickwerk.")}` : ""}`}
        disabled={checking}
        onClick={() => {
          setError("");
          void api
            .checkUpdates()
            .catch((reason) => setError(errorText(reason)));
        }}
      >
        <RefreshCw size={12} className={checking ? "spin" : undefined} />
        {t(label)}
      </button>
      {error && <span role="status">{t(error)}</span>}
    </span>
  );
}
