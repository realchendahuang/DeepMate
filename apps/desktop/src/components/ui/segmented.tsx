import { cn } from "../../lib/utils";

type SegmentedVariant = "compact" | "full";

// A single-select tab control. Compact is used for local modes and preferences;
// full gives page-level sections equal-width navigation.
interface SegmentedProps {
  options: string[];
  value: number;
  onChange: (index: number) => void;
  className?: string;
  variant?: SegmentedVariant;
}

export function Segmented({
  options,
  value,
  onChange,
  className,
  variant = "compact",
}: SegmentedProps) {
  const full = variant === "full";

  return (
    <div
      role="tablist"
      className={cn(
        "items-center gap-0.5 rounded-md bg-panel-2 p-0.5",
        full ? "grid w-full" : "inline-flex max-w-full overflow-x-auto",
        className,
      )}
      style={full ? { gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` } : undefined}
    >
      {options.map((option, index) => {
        const active = index === value;
        return (
          <button
            key={option}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(index)}
            className={cn(
              "h-[29px] min-w-0 rounded-sm px-3 text-body transition-colors",
              full ? "truncate" : "shrink-0 whitespace-nowrap",
              active
                ? "bg-raised font-semibold text-text shadow-sm"
                : "text-text-dim hover:bg-hover hover:text-text",
            )}
          >
            {option}
          </button>
        );
      })}
    </div>
  );
}
