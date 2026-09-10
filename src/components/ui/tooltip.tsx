import * as Primitive from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <Primitive.Provider delayDuration={400}>{children}</Primitive.Provider>
  );
}
export function Tooltip({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) {
  return (
    <Primitive.Root>
      <Primitive.Trigger asChild>{children}</Primitive.Trigger>
      <Primitive.Portal>
        <Primitive.Content sideOffset={7} className="tooltip">
          {label}
          <Primitive.Arrow />
        </Primitive.Content>
      </Primitive.Portal>
    </Primitive.Root>
  );
}
