import { t, useI18n } from "../lib/i18n";
import { useState } from "react";
import {
  Check,
  ChevronDown,
  Keyboard,
  MessageSquare,
  MousePointer2,
  Pause,
  ScanEye,
} from "lucide-react";
import type { Action, Step } from "../lib/types";
import { formatTime } from "../lib/utils";

function describe(action: Action): string {
  switch (action.type) {
    case "click":
      return `${t(action.button === "right" ? "Right click" : "Click")} (${action.x}, ${action.y})`;
    case "double_click":
      return `${t("Double click")} (${action.x}, ${action.y})`;
    case "move":
      return `${t("Move")} (${action.x}, ${action.y})`;
    case "drag":
      return `${t("Drag")} (${action.x}, ${action.y}) → (${action.x2}, ${action.y2})`;
    case "scroll":
      return `${t(action.amount > 0 ? "Scroll up" : "Scroll down")} ${Math.abs(action.amount)} (${action.x}, ${action.y})`;
    case "key":
      return [...action.modifiers, action.key]
        .map((k) =>
          k.length === 1
            ? k.toUpperCase()
            : k[0].toUpperCase() + k.slice(1).toLowerCase(),
        )
        .join(" + ");
    case "text":
      return t("Type text");
    case "wait":
      return `${t("Wait")} ${action.duration_ms} ms`;
    case "observe":
      return t("Check the screen");
    case "ask_user":
      return action.question;
    case "finish":
      return action.summary;
  }
}
export function Timeline({ steps }: { steps: Step[] }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? steps : steps.slice(-4);
  if (!steps.length) return null;
  return (
    <section className="timeline" aria-label={t("Action history")}>
      <div className="section-heading">
        <h3>{t("What happened")}</h3>
        <span>
          {steps.length} {steps.length === 1 ? t("entry") : t("entries")}
        </span>
      </div>
      {steps.length > 4 && (
        <button
          className="history-toggle"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded
            ? t("Show recent actions")
            : t("Show all {count} entries", { count: steps.length })}
          <ChevronDown size={13} className={expanded ? "rotated" : ""} />
        </button>
      )}
      <ol className="timeline-list">
        {visible.map((step) => {
          const action = step.action;
          const Icon =
            step.actor === "user"
              ? MessageSquare
              : step.actor === "system"
                ? Pause
                : action?.type === "text" || action?.type === "key"
                  ? Keyboard
                  : action && "x" in action
                    ? MousePointer2
                    : ScanEye;
          return (
            <li key={step.id} className={`timeline-entry actor-${step.actor}`}>
              <span className="timeline-icon">
                <Icon size={15} />
              </span>
              <div className="timeline-content">
                <div className="timeline-meta">
                  <span>
                    {step.actor === "user"
                      ? t("Your refinement")
                      : step.actor === "system"
                        ? t("Session")
                        : t("Agent")}
                  </span>
                  <time>{formatTime(step.elapsed_ms)}</time>
                  {step.actor === "agent" && (
                    <span className={`action-status status-${step.status}`}>
                      {step.status === "completed" ? (
                        <>
                          <Check size={11} />
                          {t("Sent")}
                        </>
                      ) : step.status === "pending" ? (
                        t("In progress")
                      ) : (
                        t(step.status)
                      )}
                    </span>
                  )}
                </div>
                <p>{step.description}</p>
                {action && (
                  <details className="input-details">
                    <summary>{describe(action)}</summary>
                    {action.type === "text" && <pre>{action.text}</pre>}
                    {step.image_size && (
                      <p>
                        {t("Screenshot:")}
                        {step.image_size[0]} × {step.image_size[1]} {t("px")}
                      </p>
                    )}
                    {!!step.desktop_points.length && (
                      <p>
                        {t("Desktop:")}{" "}
                        {step.desktop_points
                          .map((p) => `(${p[0]}, ${p[1]})`)
                          .join(" → ")}
                      </p>
                    )}
                    <p>
                      {step.status === "completed"
                        ? t(
                            "Input sent. The next screen check verifies the outcome.",
                          )
                        : step.status === "interrupted"
                          ? t("This action may have been only partly sent.")
                          : step.status === "skipped"
                            ? t("No input sent. The target changed.")
                            : t("Recorded in the agent’s context.")}
                    </p>
                  </details>
                )}
              </div>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
