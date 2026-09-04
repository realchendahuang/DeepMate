import * as React from "react"

import { cn } from "@/lib/utils"

// Single-line text input styled after the Slint TextField: inset surface,
// hairline border that turns accent on focus.
function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "flex h-[32px] w-full min-w-0 rounded-md border border-border bg-inset px-2.5 text-body text-text transition-colors outline-none placeholder:text-text-faint focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/20 disabled:pointer-events-none disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
}

export { Input }
