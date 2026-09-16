import * as React from "react";

import { cn } from "@/shared/lib/utils";

// Multi-line text input styled after the single-line Input: inset surface,
// hairline border that turns accent on focus. Used for raw JSON capability
// blocks and the scenario description.
function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "min-h-[80px] w-full resize-y rounded-md border border-border bg-inset px-2.5 py-2 font-mono text-body text-text transition-colors outline-none placeholder:text-text-faint focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/20 disabled:pointer-events-none disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

export { Textarea };
