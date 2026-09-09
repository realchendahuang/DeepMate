// An expandable "advanced" group and a JSON textarea for the opaque
// capability blocks (provider/model `compat`, model `reasoning_efforts`) that
// have no fixed schema: the raw JSON string is the only faithful editor.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Textarea } from "@/shared/ui/textarea";
import { Field } from "./field";
import { jsonValid } from "./json-utils";

// A collapsible advanced section inside a dialog or detail column.
export function AdvancedSection({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <div className="overflow-hidden rounded-md border border-border">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between px-3 py-2 text-small font-semibold text-text-dim transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
        aria-expanded={open}
      >
        {t("settings.advanced")}
        <ChevronDown
          className={cn("h-3.5 w-3.5 transition-transform", open && "rotate-180")}
        />
      </button>
      {open && <div className="space-y-3 border-t border-border p-3">{children}</div>}
    </div>
  );
}

// A labeled JSON textarea validated live; `invalid` lets the caller refuse
// the save. An empty draft means "no block" (null on the wire), clearing any
// previously stored value.
export function JsonField({
  label,
  hint,
  value,
  onChange,
  placeholder = '{ "key": "value" }',
}: {
  label: string;
  hint?: string;
  value: string;
  onChange: (text: string) => void;
  placeholder?: string;
}) {
  const { t } = useTranslation();
  const invalid = !jsonValid(value);

  return (
    <Field label={label}>
      <Textarea
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        spellCheck={false}
        className={cn("h-20", invalid && "border-warn focus-visible:border-warn")}
      />
      <p className={cn("text-caption", invalid ? "text-warn" : "text-text-faint")}>
        {invalid ? t("settings.invalidJson") : (hint ?? "")}
      </p>
    </Field>
  );
}