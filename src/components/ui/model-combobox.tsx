import * as Popper from "@radix-ui/react-popper";
import { Portal } from "@radix-ui/react-portal";
import { DismissableLayer } from "@radix-ui/react-dismissable-layer";
import { Check, ChevronDown, LoaderCircle } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useI18n } from "../../lib/i18n";

export function ModelCombobox({
  value,
  models,
  loading,
  onValueChange,
  onRequestModels,
}: {
  value: string;
  models: string[];
  loading: boolean;
  onValueChange: (value: string) => void;
  onRequestModels: () => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const anchor = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const listId = useId();
  const matches = models.filter((model) =>
    model.toLowerCase().includes(query.toLowerCase()),
  );
  const activeIndex = active === null ? -1 : matches.indexOf(active);

  useEffect(() => {
    if (open && activeIndex >= 0)
      list.current?.children[activeIndex]?.scrollIntoView({ block: "nearest" });
  }, [open, activeIndex]);

  function show() {
    setQuery("");
    setActive(null);
    setOpen(true);
    onRequestModels();
  }

  function choose(model: string) {
    onValueChange(model);
    setOpen(false);
    setActive(null);
    input.current?.focus();
  }

  return (
    <Popper.Root>
      <Popper.Anchor asChild>
        <div className="model-combobox" ref={anchor}>
          <input
            ref={input}
            id="model-id"
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={open}
            aria-controls={open ? listId : undefined}
            aria-activedescendant={
              open && activeIndex >= 0 ? `${listId}-${activeIndex}` : undefined
            }
            aria-describedby="model-hint"
            spellCheck={false}
            autoComplete="off"
            value={value}
            placeholder={t("Choose or enter a model ID")}
            onFocus={show}
            onChange={(event) => {
              onValueChange(event.target.value);
              setQuery(event.target.value);
              setActive(null);
              setOpen(true);
            }}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing) return;
              if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                if (!open) {
                  show();
                } else if (matches.length) {
                  const direction = event.key === "ArrowDown" ? 1 : -1;
                  const next =
                    activeIndex < 0
                      ? direction === 1
                        ? 0
                        : matches.length - 1
                      : (activeIndex + direction + matches.length) %
                        matches.length;
                  setActive(matches[next]);
                }
              } else if (event.key === "Enter" && open && activeIndex >= 0) {
                event.preventDefault();
                choose(matches[activeIndex]);
              } else if (event.key === "Escape" && open) {
                event.preventDefault();
                event.stopPropagation();
                setOpen(false);
              } else if (event.key === "Tab") {
                setOpen(false);
              }
            }}
          />
          <button
            type="button"
            aria-label={t("Show models")}
            aria-haspopup="listbox"
            aria-expanded={open}
            aria-controls={open ? listId : undefined}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              input.current?.focus();
              if (open) setOpen(false);
              else show();
            }}
          >
            {loading ? (
              <LoaderCircle size={16} className="spin" />
            ) : (
              <ChevronDown size={16} />
            )}
          </button>
        </div>
      </Popper.Anchor>
      {open && (
        <Portal role="region" aria-label={t("Available models")}>
          <DismissableLayer
            asChild
            onDismiss={() => setOpen(false)}
            onInteractOutside={(event) => {
              if (anchor.current?.contains(event.target as Node))
                event.preventDefault();
            }}
          >
            <Popper.Content
              className="dropdown-content model-options"
              role="presentation"
              align="start"
              sideOffset={6}
              collisionPadding={12}
            >
              <div
                className="dropdown-options"
                id={listId}
                ref={list}
                role="listbox"
                aria-label={t("Available models")}
              >
                {matches.map((model, index) => (
                  <div
                    id={`${listId}-${index}`}
                    key={model}
                    role="option"
                    aria-selected={model === value}
                    data-highlighted={activeIndex === index ? "" : undefined}
                    data-state={model === value ? "checked" : "unchecked"}
                    className="dropdown-option"
                    onPointerMove={() => setActive(model)}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => choose(model)}
                  >
                    <span className="dropdown-option-copy">{model}</span>
                    {model === value && (
                      <Check size={16} className="dropdown-check" />
                    )}
                  </div>
                ))}
              </div>
              {matches.length === 0 && (
                <p className="dropdown-empty" role="status">
                  {loading
                    ? t("Loading models…")
                    : t("No matching models. You can enter an exact model ID.")}
                </p>
              )}
            </Popper.Content>
          </DismissableLayer>
        </Portal>
      )}
    </Popper.Root>
  );
}
