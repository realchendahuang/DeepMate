import * as React from "react";
import { cn } from "../../lib/utils";

// Single-line text input styled after the Slint TextField: inset surface,
// hairline border that turns accent on focus.
const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, type, ...props }, ref) => (
    <input
      type={type}
      className={cn(
        "flex h-[32px] w-full rounded-md border border-border bg-inset px-2.5 text-body text-text placeholder:text-text-faint focus:outline-none focus:border-accent focus:ring-2 focus:ring-accent/20",
        className,
      )}
      ref={ref}
      {...props}
    />
  ),
);
Input.displayName = "Input";

export { Input };
