import * as React from "react"

import { cn } from "@/lib/utils"

// Multi-line text input, styled like the single-line Input: inset surface,
// hairline border that turns accent on focus.
function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex min-h-16 w-full rounded-md border border-border bg-inset px-2.5 py-1.5 text-body text-text transition-colors outline-none placeholder:text-text-faint focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/20 disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
