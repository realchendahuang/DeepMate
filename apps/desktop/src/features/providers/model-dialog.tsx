// Model editor dialog: id, context/max-token windows and the input
// modalities. Models always belong to a provider; editing keeps the model's
// own provider, creating uses the provider the dialog was opened from.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useProviderStore } from "@/app/store/providers";
import type { Model } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Field } from "./field";

// Input modalities offered as chips; "text" is the harness baseline and
// cannot be removed, so its chip is locked checked.
const MODEL_MODALITIES = ["text", "image", "video", "pdf"] as const;

// Small toggleable pill for the model dialog's input modality.
function ModalityChip({
  active,
  locked,
  label,
  onToggle,
}: {
  active: boolean;
  locked?: boolean;
  label: string;
  onToggle?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={locked ? undefined : onToggle}
      disabled={locked}
      aria-pressed={active}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-body font-medium transition-colors",
        active ? "border-accent/40 bg-accent/10 text-text" : "border-border bg-panel-2 text-text-dim",
        !locked && "hover:bg-hover",
        locked && "cursor-default opacity-90",
      )}
    >
      <span
        className={cn(
          "flex h-3.5 w-3.5 items-center justify-center rounded-sm border",
          active ? "border-accent bg-accent text-on-accent" : "border-border-strong bg-inset",
        )}
      >
        {active && (
          <svg viewBox="0 0 10 8" className="h-2 w-2 fill-none stroke-current stroke-[1.5]">
            <path d="M1 4l2.5 2.5L9 1" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
      </span>
      {label}
      {locked && (
        <svg viewBox="0 0 24 24" className="h-3 w-3 fill-none stroke-current stroke-[2]">
          <rect x="5" y="11" width="14" height="9" rx="2" />
          <path d="M8 11V8a4 4 0 018 0v3" />
        </svg>
      )}
    </button>
  );
}

export function ModelDialog({
  profile,
  open,
  value,
  defaultProvider,
  onClose,
}: {
  profile: string;
  open: boolean;
  value: Model | null;
  defaultProvider: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const upsertModel = useProviderStore((s) => s.upsertModel);

  // Models always belong to a provider; editing keeps the model's own
  // provider, creating uses the provider the dialog was opened from.
  const provider = value?.provider ?? defaultProvider;
  const [id, setId] = useState(value?.id ?? "");
  const [contextWindow, setContextWindow] = useState(
    value?.context_window ? String(value.context_window) : "",
  );
  const [maxTokens, setMaxTokens] = useState(value?.max_tokens ? String(value.max_tokens) : "");
  const [modalities, setModalities] = useState<string[]>(() => {
    const declared = value?.input;
    if (declared && declared.length > 0) return declared;
    return ["text"];
  });

  const canSave = provider.trim().length > 0 && id.trim().length > 0;

  // Numeric fields accept digits only.
  const digitsOnly = (value: string) => value.replace(/\D/g, "");

  const toggleModality = (modality: string) => {
    setModalities((current) =>
      current.includes(modality)
        ? current.filter((candidate) => candidate !== modality)
        : [...current, modality],
    );
  };

  const save = async () => {
    if (!canSave) return;
    // The display name mirrors the id unless the caller edits YAML directly;
    // the harness treats a missing name as the id anyway.
    await upsertModel(profile, provider.trim(), {
      id: id.trim(),
      name: id.trim(),
      provider: provider.trim(),
      context_window: contextWindow.trim() ? Number(contextWindow.trim()) : null,
      max_tokens: maxTokens.trim() ? Number(maxTokens.trim()) : null,
      input: modalities.length > 0 ? modalities : null,
      reasoning_efforts: value?.reasoning_efforts ?? null,
      compat: value?.compat ?? null,
    });
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{value ? t("settings.editModel") : t("settings.addModel")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <Field label={t("settings.modelId")}>
            <Input
              value={id}
              onChange={(event) => setId(event.target.value)}
              placeholder="deepseek-chat"
              spellCheck={false}
              className="font-mono"
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label={t("settings.contextWindow")}>
              <Input
                value={contextWindow}
                onChange={(event) => setContextWindow(digitsOnly(event.target.value))}
                placeholder="262144"
                inputMode="numeric"
              />
            </Field>
            <Field label={t("settings.maxTokens")}>
              <Input
                value={maxTokens}
                onChange={(event) => setMaxTokens(digitsOnly(event.target.value))}
                placeholder="32768"
                inputMode="numeric"
              />
            </Field>
          </div>
          <Field label={t("settings.input")}>
            <div className="flex flex-wrap gap-1.5">
              {MODEL_MODALITIES.map((modality) => (
                <ModalityChip
                  key={modality}
                  active={modalities.includes(modality)}
                  locked={modality === "text"}
                  label={t(`settings.modality.${modality}`)}
                  onToggle={() => toggleModality(modality)}
                />
              ))}
            </div>
          </Field>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("settings.cancel")}
          </Button>
          <Button variant="primary" onClick={save} disabled={!canSave}>
            {t("settings.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
