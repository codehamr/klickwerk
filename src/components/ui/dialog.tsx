import { useI18n } from "../../lib/i18n";
import * as Primitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";

export function Dialog({
  children,
  ...props
}: ComponentProps<typeof Primitive.Root>) {
  return <Primitive.Root {...props}>{children}</Primitive.Root>;
}
export function DialogContent({
  children,
  title,
  description,
  onCloseAutoFocus,
  closeLabel = "Close dialog",
}: {
  children: ReactNode;
  title: string;
  description: string;
  onCloseAutoFocus?: (event: Event) => void;
  closeLabel?: string;
}) {
  const { t } = useI18n();
  return (
    <Primitive.Portal>
      <Primitive.Overlay className="dialog-overlay" />
      <Primitive.Content
        className="dialog-content"
        onCloseAutoFocus={onCloseAutoFocus}
      >
        <div className="dialog-heading">
          <div>
            <Primitive.Title>{title}</Primitive.Title>
            <Primitive.Description>{description}</Primitive.Description>
          </div>
          <Primitive.Close
            className="button button-ghost button-icon"
            aria-label={t(closeLabel)}
          >
            <X size={19} />
          </Primitive.Close>
        </div>
        {children}
      </Primitive.Content>
    </Primitive.Portal>
  );
}
