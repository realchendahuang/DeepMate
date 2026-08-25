import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

// The one button used across the app, styled after the Slint DmButton.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-md text-body font-medium transition-colors focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary:
          "bg-accent text-on-accent shadow-accent-glow hover:bg-accent/90",
        secondary:
          "border border-border bg-panel-2 text-text hover:bg-hover",
        ghost: "text-text-dim hover:bg-hover hover:text-text",
        danger: "text-fail hover:bg-fail/10",
      },
      size: {
        default: "h-[30px] px-4",
        sm: "h-[26px] px-3 text-small",
        icon: "h-[30px] w-[30px]",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => (
    <button
      className={cn(buttonVariants({ variant, size }), className)}
      ref={ref}
      {...props}
    />
  ),
);
Button.displayName = "Button";

export { Button, buttonVariants };
