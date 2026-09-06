// The per-scenario provider & model editor, moved out of the old global
// Settings page. Every store call is scoped to the given scenario profile:
// each scenario owns its providers/models (its own settings document).

import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Plus,
  Pencil,
  SquarePen,
  Trash2,
  Eye,
  EyeOff,
  Box,
  Cpu,
  Cloud,
} from "lucide-react";
import { useStore } from "../../store";
import type { Model, Provider } from "../../api";
import { cn } from "../../lib/utils";
import { Card } from "../ui/card";
import { EmptyState } from "../ui/empty-state";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

// Compact numeric badge on a model row ("1M", "262.1K").
function formatTokens(value: number): string {
  if (value >= 1_000_000) {
    const n = value / 1_000_000;
    return `${n >= 100 ? Math.round(n) : Math.round(n * 10) / 10}M`;
  }
  if (value >= 1_000) {
    const n = value / 1_000;
    return `${n >= 100 ? Math.round(n) : Math.round(n * 10) / 10}K`;
  }
  return String(value);
}

function TokenBadge({ tokens }: { tokens: number }) {
  return (
    <span className="inline-flex shrink-0 items-center rounded-full border border-border bg-panel-2 px-1.5 py-0.5 font-mono text-caption font-semibold tabular-nums text-text-dim">
      {formatTokens(tokens)}
    </span>
  );
}

// Small toggleable pill for the model dialog's input modality (text/image/...).
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


// ---- Models page: split panel ----

// Wire protocols a provider can speak. The first option (empty) keeps the
// harness default; the others cover the endpoints the harness ships support.
const API_PROTOCOLS = ["openai-responses", "openai-completions"] as const;
const DEEPSEEK_PROVIDER_ID = "deepseek-official";

// A group heading inside the provider sidebar.
function SidebarGroupLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-3 pt-4 pb-1.5 text-caption font-semibold text-text-faint">{children}</div>
  );
}

// One provider row in the sidebar: icon, name, count. Selecting it loads the
// detail column; edit/delete live in the detail header, not here.
function ProviderRow({
  provider,
  active,
  onSelect,
}: {
  provider: Provider;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect();
        }
      }}
      className={cn(
        "flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-body font-medium transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent",
        active && "bg-accent-soft",
      )}
      aria-current={active ? "true" : undefined}
    >
      <Box className={cn("h-4 w-4 shrink-0", active ? "text-accent" : "text-text-faint")} />
      <span className={cn("min-w-0 flex-1 truncate", active ? "text-accent" : "text-text")}>
        {provider.name}
      </span>
    </div>
  );
}

// The whole models page: a full-height provider sidebar on the left and the
// inline provider detail (Base URL / API format / API key) plus its model
// list on the right. Both columns live inside one bordered surface; each
// column scrolls independently on short windows.
function ModelsPanel({
  profile,
  providers,
  models,
  selectedId,
  onSelect,
  onNewProvider,
  onDeleteProvider,
  onNewModel,
  onEditModel,
  onDeleteModel,
}: {
  profile: string;
  providers: Provider[];
  models: Model[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onNewProvider: () => void;
  onDeleteProvider: (provider: Provider) => void;
  onNewModel: () => void;
  onEditModel: (model: Model) => void;
  onDeleteModel: (model: Model) => void;
}) {
  const { t } = useTranslation();
  const upsertProvider = useStore((s) => s.upsertProvider);

  const selected = providers.find((provider) => provider.id === selectedId) ?? null;
  const providerModels = useMemo(
    () => (selected ? models.filter((model) => model.provider === selected.id) : []),
    [models, selected],
  );

  // Inline provider form state, reseeded per selection via the key on the
  // detail column.
  const [name, setName] = useState(selected?.name ?? "");
  const [api, setApi] = useState(selected?.api ?? "");
  const [baseUrl, setBaseUrl] = useState(selected?.base_url ?? "");
  const [apiKeyEnv, setApiKeyEnv] = useState(selected?.api_key_env ?? "");
  const [keyVisible, setKeyVisible] = useState(false);
  const [editingName, setEditingName] = useState(false);

  // Legacy configs may carry a protocol not in the select; treat them as
  // "auto" so the select always shows a valid option.
  const apiValue = API_PROTOCOLS.includes(api as (typeof API_PROTOCOLS)[number])
    ? api
    : "__default__";

  const isDeepseek = selected?.id === DEEPSEEK_PROVIDER_ID;
  const dirty =
    !!selected &&
    (name !== selected.name ||
      api !== (selected.api ?? "") ||
      baseUrl !== (selected.base_url ?? "") ||
      apiKeyEnv !== (selected.api_key_env ?? ""));

  const saveProvider = async () => {
    if (!selected || !name.trim()) return;
    await upsertProvider(profile, {
      id: selected.id,
      name: name.trim(),
      kind: selected.kind,
      api: api.trim() || null,
      base_url: baseUrl.trim() || null,
      api_key_env: apiKeyEnv.trim() || null,
      compat: selected.compat,
    });
  };

  return (
    <Card className="overflow-hidden p-0">
      <div className="flex flex-col md:h-[calc(100dvh-160px)] md:min-h-[420px] md:flex-row">
        {/* Provider sidebar: grouped list, fills the panel height. */}
        <div className="flex w-full shrink-0 flex-col border-b border-border bg-sidebar md:w-[220px] md:border-b-0 md:border-r">
          <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
            <SidebarGroupLabel>{t("settings.groupDefault")}</SidebarGroupLabel>
            {providers
              .filter((provider) => provider.id === DEEPSEEK_PROVIDER_ID)
              .map((provider) => (
                <ProviderRow
                  key={provider.id}
                  provider={provider}
                  active={provider.id === selectedId}
                  onSelect={() => onSelect(provider.id)}
                />
              ))}
            {providers.some((provider) => provider.id !== DEEPSEEK_PROVIDER_ID) && (
              <SidebarGroupLabel>{t("settings.groupCustom")}</SidebarGroupLabel>
            )}
            <div className="space-y-0.5">
              {providers
                .filter((provider) => provider.id !== DEEPSEEK_PROVIDER_ID)
                .map((provider) => (
                  <ProviderRow
                    key={provider.id}
                    provider={provider}
                    active={provider.id === selectedId}
                    onSelect={() => onSelect(provider.id)}
                  />
                ))}
            </div>
          </div>
          <div className="border-t border-border p-2">
            <button
              type="button"
              onClick={onNewProvider}
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-body font-medium text-text-dim transition-colors hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
            >
              <Plus className="h-4 w-4 shrink-0" />
              {t("settings.addProvider")}
            </button>
          </div>
        </div>

        {/* Detail column: inline provider settings + model list. */}
        {selected ? (
          <div
            key={selected.id}
            className="min-w-0 flex-1 overflow-y-auto p-4 md:p-5"
          >
            {/* Provider header: name (inline rename), edit affordance, delete. */}
            <div className="flex items-center gap-2">
              {editingName ? (
                <Input
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  onBlur={() => setEditingName(false)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") setEditingName(false);
                    if (event.key === "Escape") {
                      setName(selected.name);
                      setEditingName(false);
                    }
                  }}
                  className="h-8 max-w-[240px] text-title font-bold"
                  autoFocus
                  aria-label={t("settings.name")}
                />
              ) : (
                <h2 className="truncate text-title font-bold text-text">
                  {selected.id === DEEPSEEK_PROVIDER_ID ? selected.name : name || selected.name}
                </h2>
              )}
              {!isDeepseek && (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  onClick={() => setEditingName(true)}
                  aria-label={t("settings.rename")}
                >
                  <SquarePen className="h-3.5 w-3.5" />
                </Button>
              )}
              {!isDeepseek && (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  onClick={() => onDeleteProvider(selected)}
                  aria-label={t("settings.remove")}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              )}
              {dirty && (
                <Button
                  type="button"
                  variant="primary"
                  size="sm"
                  className="ml-auto"
                  onClick={saveProvider}
                  disabled={!name.trim()}
                >
                  {t("settings.save")}
                </Button>
              )}
            </div>

            {/* Provider settings, edited in place. */}
            <div className="mt-4 max-w-[560px] space-y-3.5">
              <Field label={t("settings.baseUrl")}>
                <Input
                  value={baseUrl}
                  onChange={(event) => setBaseUrl(event.target.value)}
                  placeholder="https://api.example.com/v1"
                  spellCheck={false}
                />
              </Field>
              <Field label={t("settings.api")}>
                <Select
                  value={apiValue}
                  onValueChange={(value) => setApi(value === "__default__" ? "" : value)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__default__">{t("settings.apiDefault")}</SelectItem>
                    {API_PROTOCOLS.map((protocol) => (
                      <SelectItem key={protocol} value={protocol}>
                        {protocol}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              <div className="space-y-1">
                <Label>{t("settings.apiKeyEnv")}</Label>
                <div className="relative">
                  <Input
                    type={keyVisible ? "text" : "password"}
                    value={apiKeyEnv}
                    onChange={(event) => setApiKeyEnv(event.target.value)}
                    placeholder={isDeepseek ? "DEEPSEEK_API_KEY" : "MY_API_KEY"}
                    className="pr-9"
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute right-0.5 top-1/2 h-6 w-6 -translate-y-1/2"
                    onClick={() => setKeyVisible(!keyVisible)}
                    aria-label={t(keyVisible ? "settings.hideKey" : "settings.showKey")}
                    title={t(keyVisible ? "settings.hideKey" : "settings.showKey")}
                    tabIndex={-1}
                  >
                    {keyVisible ? (
                      <EyeOff className="h-3.5 w-3.5" />
                    ) : (
                      <Eye className="h-3.5 w-3.5" />
                    )}
                  </Button>
                </div>
                <p className="text-caption text-text-faint">{t("settings.apiKeyHint")}</p>
              </div>
            </div>

            {/* Models of this provider. */}
            <div className="mt-6 flex items-center justify-between gap-3">
              <h3 className="text-heading font-semibold text-text">{t("settings.models")}</h3>
              <Button type="button" variant="secondary" size="sm" onClick={onNewModel}>
                <Plus className="h-4 w-4" />
                {t("settings.addModel")}
              </Button>
            </div>
            {providerModels.length === 0 ? (
              <div className="mt-3">
                <EmptyState icon={<Cpu className="h-8 w-8 text-text-faint" />}>
                  {t("settings.noModels")}
                </EmptyState>
              </div>
            ) : (
              <div className="mt-3 overflow-hidden rounded-md border border-border">
                <div className="divide-y divide-border">
                  {providerModels.map((model) => (
                    <div
                      key={model.id}
                      className="flex items-center gap-2 px-3 py-2 transition-colors hover:bg-hover"
                    >
                      <div className="min-w-0 flex-1 truncate font-mono text-body text-text">
                        {model.id}
                      </div>
                      <div className="flex shrink-0 items-center gap-1">
                        {model.input?.includes("image") && (
                          <span className="inline-flex shrink-0 items-center rounded-full border border-border bg-panel-2 px-1.5 py-0.5 text-caption text-text-dim">
                            {t("settings.badgeVision")}
                          </span>
                        )}
                        {model.context_window != null && (
                          <TokenBadge tokens={model.context_window} />
                        )}
                      </div>
                      <div className="flex shrink-0 items-center gap-0.5">
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="h-7 w-7"
                          onClick={() => onEditModel(model)}
                          aria-label={t("settings.editModel")}
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="h-7 w-7"
                          onClick={() => onDeleteModel(model)}
                          aria-label={t("settings.remove")}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="flex min-w-0 flex-1 items-center justify-center p-6">
            <EmptyState icon={<Cloud className="h-8 w-8 text-text-faint" />}>
              {t("settings.noProviders")}
            </EmptyState>
          </div>
        )}
      </div>
    </Card>
  );
}

// ---- New provider dialog ----

// Creating a provider only needs an id and a display name; the connection
// fields (Base URL / API format / key) are edited inline on the detail side
// right after creation.
function NewProviderDialog({
  profile,
  open,
  onClose,
  onCreated,
}: {
  profile: string;
  open: boolean;
  onClose: () => void;
  onCreated: (id: string) => void;
}) {
  const { t } = useTranslation();
  const upsertProvider = useStore((s) => s.upsertProvider);

  const [id, setId] = useState("");
  const [name, setName] = useState("");

  const canSave = id.trim().length > 0 && name.trim().length > 0;

  const save = async () => {
    if (!canSave) return;
    await upsertProvider(profile, {
      id: id.trim(),
      name: name.trim(),
      kind: "pi-ai",
      api: null,
      base_url: null,
      api_key_env: null,
      compat: null,
    });
    onCreated(id.trim());
    setId("");
    setName("");
    onClose();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          setId("");
          setName("");
          onClose();
        }
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.addProvider")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <Field label={t("settings.id")}>
            <Input
              value={id}
              onChange={(event) => setId(event.target.value)}
              placeholder="openai"
              spellCheck={false}
              autoFocus
            />
          </Field>
          <Field label={t("settings.name")}>
            <Input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="OpenAI"
            />
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

// ---- Model editor ----

// Input modalities offered as chips; "text" is the harness baseline and
// cannot be removed, so its chip is locked checked.
const MODEL_MODALITIES = ["text", "image", "video", "pdf"] as const;

function ModelDialog({
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
  const upsertModel = useStore((s) => s.upsertModel);

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

// Wired store accessors so the page reads from one source of truth. Kept at
// the bottom to keep the render functions focused on layout.

export {
  ModelsPanel,
  NewProviderDialog,
  ModelDialog,
};
