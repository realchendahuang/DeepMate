import * as React from "react";
import { Label as LabelPrimitive } from "radix-ui";

import { cn } from "@/shared/lib/utils";

// A labelled form row label, styled after the Slint field captions.
function Label({ className, ...props }: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn(
        "flex items-center gap-2 text-caption leading-none font-bold text-text-dim select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

export { Label };
