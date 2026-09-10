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
      return `${action.button === "right" ? "Right click" : "Click"} at (${action.x}, ${action.y})`;
    case "double_click":
      return `Double click at (${action.x}, ${action.y})`;
    case "move":
      return `Move to (${action.x}, ${action.y})`;
    case "drag":
      return `Drag (${action.x}, ${action.y}) → (${action.x2}, ${action.y2})`;
    case "scroll":
      return `Scroll ${action.amount > 0 ? "up" : "down"} ${Math.abs(action.amount)} at (${action.x}, ${action.y})`;
    case "key":
      return [...action.modifiers, action.key]
        .map((k) =>
          k.length === 1
            ? k.toUpperCase()
            : k[0].toUpperCase() + k.slice(1).toLowerCase(),
        )
        .join(" + ");
    case "text":
      return "Type text";
    case "wait":
      return `Wait ${action.duration_ms} ms`;
    case "observe":
      return "Check the screen";
    case "ask_user":
      return action.question;
    case "finish":
      return action.summary;
  }
}
export function Timeline({ steps }: { steps: Step[] }) {
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? steps : steps.slice(-4);
  if (!steps.length) return null;
  return (
    <section className="timeline" aria-label="Action history">
      <div className="section-heading">
        <h3>What happened</h3>
        <span>
          {steps.length} {steps.length === 1 ? "entry" : "entries"}
        </span>
      </div>
      {steps.length > 4 && (
        <button
          className="history-toggle"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded
            ? "Show recent actions"
            : `Show all ${steps.length} entries`}
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
                      ? "Your refinement"
                      : step.actor === "system"
                        ? "Session"
                        : "Agent"}
                  </span>
                  <time>{formatTime(step.elapsed_ms)}</time>
                  {step.actor === "agent" && (
                    <span className={`action-status status-${step.status}`}>
                      {step.status === "completed" ? (
                        <>
                          <Check size={11} /> Sent
                        </>
                      ) : step.status === "pending" ? (
                        "In progress"
                      ) : (
                        step.status
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
                        Screenshot: {step.image_size[0]} × {step.image_size[1]}{" "}
                        px
                      </p>
                    )}
                    {!!step.desktop_points.length && (
                      <p>
                        Desktop:{" "}
                        {step.desktop_points
                          .map((p) => `(${p[0]}, ${p[1]})`)
                          .join(" → ")}
                      </p>
                    )}
                    <p>
                      {step.status === "completed"
                        ? "Input sent. The next screen check verifies the outcome."
                        : step.status === "interrupted"
                          ? "This action may have been only partly sent."
                          : step.status === "skipped"
                            ? "No input sent. The target changed."
                            : "Recorded in the agent’s context."}
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
