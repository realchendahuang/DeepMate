import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

// The design system defines custom font-size tokens (`text-body`,
// `text-small`, …) that tailwind-merge would otherwise classify as color
// utilities and merge away — a `size="sm"` button carrying both
// `text-on-accent` (color) and `text-small` (font-size) silently lost the
// color class, rendering dark text on the accent background. Teaching
// twMerge the token names keeps the two groups apart, so a size class can
// never drop a variant's color class again.
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [
        "text-display",
        "text-title",
        "text-heading",
        "text-body",
        "text-small",
        "text-caption",
        "text-brand",
      ],
    },
  },
});

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
