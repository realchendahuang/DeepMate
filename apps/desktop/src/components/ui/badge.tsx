import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

// Small status pill, styled after the Slint Badge. The tone vocabulary
// (pass | warn | fail | skip | accent | neutral) drives the color everywhere.
const badgeVariants = cva(
  "inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-caption font-semibold",
  {
    variants: {
      variant: {
        pass: "border-pass/25 bg-pass/10 text-pass",
        warn: "border-warn/25 bg-warn/10 text-warn",
        fail: "border-fail/25 bg-fail/10 text-fail",
        skip: "border-skip/25 bg-skip/10 text-skip",
        accent: "border-accent/25 bg-accent/10 text-accent",
        neutral: "border-neutral/25 bg-neutral/10 text-neutral",
      },
    },
    defaultVariants: {
      variant: "neutral",
    },
  }
)

function Badge({
  className,
  variant = "neutral",
  dot = true,
  asChild = false,
  children,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { dot?: boolean; asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-variant={variant}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    >
      {dot && (
        <span
          className={cn("h-1.5 w-1.5 rounded-full", {
            "bg-pass": variant === "pass",
            "bg-warn": variant === "warn",
            "bg-fail": variant === "fail",
            "bg-skip": variant === "skip",
            "bg-accent": variant === "accent",
            "bg-neutral": variant === "neutral",
          })}
        />
      )}
      {children}
    </Comp>
  )
}

export { Badge, badgeVariants }
