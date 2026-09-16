import * as React from "react";
import { Switch as SwitchPrimitive } from "radix-ui";

import { cn } from "@/shared/lib/utils";

// A boolean toggle used by the preference rows (auto-start, close-to-tray,
// update checks). Styled with the same tokens as the rest of the UI.
function Switch({ className, ...props }: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full border transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50 data-checked:border-accent data-checked:bg-accent data-unchecked:border-border-strong data-unchecked:bg-inset data-disabled:cursor-not-allowed data-disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block size-4 rounded-full bg-text-dim shadow-sm transition-transform data-checked:bg-on-accent data-checked:translate-x-[18px] data-unchecked:translate-x-[2px]"
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
