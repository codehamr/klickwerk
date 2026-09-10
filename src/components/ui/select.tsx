import * as Primitive from "@radix-ui/react-select";
import { Check, ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";
import { useI18n } from "../../lib/i18n";

interface Option {
  value: string;
  label: string;
  description?: string;
}

export function Select({
  id,
  value,
  options,
  onValueChange,
}: {
  id: string;
  value: string;
  options: Option[];
  onValueChange: (value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const { t } = useI18n();
  return (
    <Primitive.Root
      value={value}
      onValueChange={onValueChange}
      open={open}
      onOpenChange={setOpen}
    >
      <Primitive.Trigger
        id={id}
        className="select-trigger"
        aria-labelledby={`${id}-label`}
        aria-describedby={`${id}-hint`}
      >
        <Primitive.Value />
        <Primitive.Icon className="select-chevron">
          <ChevronDown size={15} />
        </Primitive.Icon>
      </Primitive.Trigger>
      <Primitive.Portal>
        <div role="region" aria-label={t("Selection options")} hidden={!open}>
          <Primitive.Content
            className="dropdown-content"
            position="popper"
            align="end"
            sideOffset={6}
            collisionPadding={12}
          >
            <Primitive.ScrollUpButton className="select-scroll">
              <ChevronUp size={15} />
            </Primitive.ScrollUpButton>
            <Primitive.Viewport className="dropdown-options">
              {options.map((option) => (
                <Primitive.Item
                  key={option.value}
                  value={option.value}
                  textValue={option.label}
                  className="dropdown-option"
                >
                  <span className="dropdown-option-copy">
                    <Primitive.ItemText>{option.label}</Primitive.ItemText>
                    {option.description && <small>{option.description}</small>}
                  </span>
                  <Primitive.ItemIndicator className="dropdown-check">
                    <Check size={16} />
                  </Primitive.ItemIndicator>
                </Primitive.Item>
              ))}
            </Primitive.Viewport>
            <Primitive.ScrollDownButton className="select-scroll">
              <ChevronDown size={15} />
            </Primitive.ScrollDownButton>
          </Primitive.Content>
        </div>
      </Primitive.Portal>
    </Primitive.Root>
  );
}
