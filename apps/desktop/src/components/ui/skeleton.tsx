import { cn } from "@/lib/utils"

// A muted placeholder block for loading states.
function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn("animate-pulse rounded-md bg-inset", className)}
      {...props}
    />
  )
}

export { Skeleton }
