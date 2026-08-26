import { cn } from "../../lib/utils";

// A boolean toggle used by the preference rows (auto-start, close-to-tray,
// update checks). Styled with the same tokens as the rest of the UI.
interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  "aria-label"?: string;
}

export function Switch({ checked, onChange, disabled, ...props }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full border transition-colors focus-visible:outline-none",
        checked ? "border-accent bg-accent" : "border-border-strong bg-inset",
        disabled && "opacity-50",
      )}
      {...props}
    >
      <span
        className={cn(
          "inline-block h-[16px] w-[16px] rounded-full bg-white shadow-sm transition-transform",
          checked ? "translate-x-[18px]" : "translate-x-[2px]",
        )}
      />
    </button>
  );
}
