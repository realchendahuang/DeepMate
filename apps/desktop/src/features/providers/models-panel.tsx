// The provider & model editor panel: a full-height provider sidebar on the
// left and the inline provider detail (Base URL / API format / API key) plus
// its model list on the right. Both columns live inside one bordered
// surface; each column scrolls independently on short windows.

import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Box,
  Check,
  Cloud,
  Copy,
  Cpu,
  Eye,
  EyeOff,
  Plus,
  Search,
  SquarePen,
  Trash2,
  Pencil,
} from "lucide-react";
import { useProviderStore } from "@/app/store/providers";
import type { Model, Provider } from "@/shared/api/api";
import { cn } from "@/shared/lib/utils";
import { Card } from "@/shared/ui/card";
import { EmptyState } from "@/shared/ui/empty-state";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Label } from "@/shared/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Field } from "./field";
import { AdvancedSection, JsonField } from "./json-field";
import { jsonOrNull, jsonValid } from "./json-utils";

// Wire protocols a provider can speak. The first option (empty) keeps the
// harness default; the others cover the endpoints the harness ships support.
const API_PROTOCOLS = ["openai-responses", "openai-completions"] as const;
const DEEPSEEK_PROVIDER_ID = "deepseek-official";

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
  modelCount,
  onSelect,
}: {
  provider: Provider;
  active: boolean;
  modelCount?: number;
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
      {modelCount !== undefined && modelCount > 0 && (
        <span
          className={cn(
            "rounded-full px-1.5 py-0.5 font-mono text-micro font-semibold tabular-nums",
            active ? "bg-accent/15 text-accent" : "bg-panel text-text-faint",
          )}
        >
          {modelCount}
        </span>
      )}
    </div>
  );
}

export function ModelsPanel({
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
  const selected = providers.find((provider) => provider.id === selectedId) ?? null;

  return (
    <Card className="flex h-full min-h-0 flex-col overflow-hidden p-0">
      <div className="flex min-h-0 flex-1 flex-col md:flex-row">
        <ProviderSidebar
          providers={providers}
          models={models}
          selectedId={selectedId}
          onSelect={onSelect}
          onNewProvider={onNewProvider}
        />
        {/* Keying the detail pane by provider id remounts its form state, so
            switching providers can never leave the previous provider's values
            (or its dirty flag) in the editor. */}
        {selected ? (
          <ProviderDetail
            key={selected.id}
            profile={profile}
            selected={selected}
            models={models}
            onNewModel={onNewModel}
            onEditModel={onEditModel}
            onDeleteModel={onDeleteModel}
            onDeleteProvider={onDeleteProvider}
          />
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

// The left column: the grouped provider list plus the "add provider" button.
function ProviderSidebar({
  providers,
  models,
  selectedId,
  onSelect,
  onNewProvider,
}: {
  providers: Provider[];
  models: Model[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onNewProvider: () => void;
}) {
  const { t } = useTranslation();

  // One pass over the model list instead of a per-row filter scan.
  const modelCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const model of models) {
      // A model without a provider belongs to no row, so it is not counted.
      if (model.provider === null) continue;
      counts.set(model.provider, (counts.get(model.provider) ?? 0) + 1);
    }
    return counts;
  }, [models]);

  const defaultProviders = providers.filter((p) => p.id === DEEPSEEK_PROVIDER_ID);
  const customProviders = providers.filter((p) => p.id !== DEEPSEEK_PROVIDER_ID);

  return (
    <div className="flex w-full shrink-0 flex-col border-b border-border bg-sidebar md:w-[220px] md:border-b-0 md:border-r">
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        <SidebarGroupLabel>{t("settings.groupDefault")}</SidebarGroupLabel>
        {defaultProviders.map((provider) => (
          <ProviderRow
            key={provider.id}
            provider={provider}
            active={provider.id === selectedId}
            modelCount={modelCounts.get(provider.id)}
            onSelect={() => onSelect(provider.id)}
          />
        ))}
        {customProviders.length > 0 && (
          <SidebarGroupLabel>{t("settings.groupCustom")}</SidebarGroupLabel>
        )}
        <div className="space-y-0.5">
          {customProviders.map((provider) => (
            <ProviderRow
              key={provider.id}
              provider={provider}
              active={provider.id === selectedId}
              modelCount={modelCounts.get(provider.id)}
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
  );
}

// The right column: editable provider settings plus the model list.
function ProviderDetail({
  profile,
  selected,
  models,
  onNewModel,
  onEditModel,
  onDeleteModel,
  onDeleteProvider,
}: {
  profile: string;
  selected: Provider;
  models: Model[];
  onNewModel: () => void;
  onEditModel: (model: Model) => void;
  onDeleteModel: (model: Model) => void;
  onDeleteProvider: (provider: Provider) => void;
}) {
  const { t } = useTranslation();
  const upsertProvider = useProviderStore((s) => s.upsertProvider);

  const providerModels = useMemo(
    () => models.filter((model) => model.provider === selected.id),
    [models, selected],
  );

  // Form state. This component is keyed by provider id (see ModelsPanel), so
  // it initializes fresh for each provider.
  const [name, setName] = useState(selected.name);
  const [api, setApi] = useState(selected.api ?? "");
  const [baseUrl, setBaseUrl] = useState(selected.base_url ?? "");
  const [apiKeyEnv, setApiKeyEnv] = useState(selected.api_key_env ?? "");
  const [compat, setCompat] = useState(selected.compat ?? "");
  const [keyVisible, setKeyVisible] = useState(false);
  const [editingName, setEditingName] = useState(false);
  const [modelSearch, setModelSearch] = useState("");
  const [copiedModelId, setCopiedModelId] = useState<string | null>(null);

  // Legacy configs may carry a protocol not in the select; treat them as
  // "auto" so the select always shows a valid option.
  const apiValue = API_PROTOCOLS.includes(api as (typeof API_PROTOCOLS)[number])
    ? api
    : "__default__";

  const isDeepseek = selected.id === DEEPSEEK_PROVIDER_ID;
  const dirty =
    name !== selected.name ||
    api !== (selected.api ?? "") ||
    baseUrl !== (selected.base_url ?? "") ||
    apiKeyEnv !== (selected.api_key_env ?? "") ||
    compat !== (selected.compat ?? "");

  const discardChanges = () => {
    setName(selected.name);
    setApi(selected.api ?? "");
    setBaseUrl(selected.base_url ?? "");
    setApiKeyEnv(selected.api_key_env ?? "");
    setCompat(selected.compat ?? "");
  };

  const filteredModels = useMemo(() => {
    if (!modelSearch.trim()) return providerModels;
    const q = modelSearch.toLowerCase();
    return providerModels.filter(
      (m) => m.id.toLowerCase().includes(q) || (m.name && m.name.toLowerCase().includes(q)),
    );
  }, [providerModels, modelSearch]);

  // Commit an inline rename from the keyboard: Enter means "save this", so
  // the draft is persisted right away rather than waiting for the Save
  // button. Escape (handled below) restores the original name.
  const commitName = () => {
    setEditingName(false);
    if (name.trim() && name !== selected.name) {
      void saveProvider().catch(() => {
        // The failure is surfaced by the store; put the original back so the
        // field does not keep a name that was never saved.
        setName(selected.name);
      });
    }
  };

  const saveProvider = async () => {
    if (!name.trim()) return;
    await upsertProvider(profile, {
      id: selected.id,
      name: name.trim(),
      kind: selected.kind,
      api: api.trim() || null,
      base_url: baseUrl.trim() || null,
      api_key_env: apiKeyEnv.trim() || null,
      compat: jsonOrNull(compat),
    });
  };

  return (
    <div className="min-w-0 flex-1 overflow-y-auto p-4 md:p-5">
      {/* Provider header: name (inline rename), edit affordance, delete. */}
      <div className="flex items-center gap-2">
        {editingName ? (
          <Input
            value={name}
            onChange={(event) => setName(event.target.value)}
            onBlur={commitName}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing) return;
              // Enter commits the inline rename, Escape restores it.
              if (event.key === "Enter") commitName();
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
            {isDeepseek ? selected.name : name || selected.name}
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
          <div className="ml-auto flex items-center gap-2">
            <span className="hidden text-caption text-text-dim sm:inline">
              {t("settings.unsavedChanges")}
            </span>
            <Button type="button" variant="ghost" size="sm" onClick={discardChanges}>
              {t("settings.discard")}
            </Button>
            <Button
              type="button"
              variant="primary"
              size="sm"
              onClick={saveProvider}
              disabled={!name.trim() || !jsonValid(compat)}
            >
              {t("settings.save")}
            </Button>
          </div>
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
              {keyVisible ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
            </Button>
          </div>
          <p className="text-caption text-text-faint">{t("settings.apiKeyHint")}</p>
        </div>
        <AdvancedSection>
          <JsonField
            label={t("settings.compat")}
            hint={t("settings.compatHint")}
            value={compat}
            onChange={setCompat}
            placeholder={t("settings.jsonPlaceholder", { key: "supportsStore" })}
          />
        </AdvancedSection>
      </div>

      {/* Models of this provider. */}
      <div className="mt-6 flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <h3 className="text-heading font-semibold text-text">{t("settings.models")}</h3>
          <span className="rounded-full bg-panel-2 px-2 py-0.5 font-mono text-caption font-semibold text-text-dim">
            {providerModels.length}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {providerModels.length > 2 && (
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-faint" />
              <Input
                value={modelSearch}
                onChange={(e) => setModelSearch(e.target.value)}
                placeholder={t("settings.searchModels")}
                className="h-7 w-36 pl-8 text-small md:w-48"
              />
            </div>
          )}
          <Button type="button" variant="secondary" size="sm" onClick={onNewModel}>
            <Plus className="h-4 w-4" />
            {t("settings.addModel")}
          </Button>
        </div>
      </div>
      {providerModels.length === 0 ? (
        <div className="mt-3">
          <EmptyState icon={<Cpu className="h-8 w-8 text-text-faint" />}>
            {t("settings.noModels")}
          </EmptyState>
        </div>
      ) : filteredModels.length === 0 ? (
        <div className="mt-3">
          <EmptyState icon={<Search className="h-8 w-8 text-text-faint" />}>
            {t("plugins.searchEmpty")}
          </EmptyState>
        </div>
      ) : (
        <div className="mt-3 overflow-hidden rounded-md border border-border">
          <div className="divide-y divide-border">
            {filteredModels.map((model) => (
              <div
                key={model.id}
                className="flex items-center gap-2 px-3 py-2 transition-colors hover:bg-hover"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-body text-text">
                    {model.name && model.name !== model.id ? model.name : model.id}
                  </div>
                  {model.name && model.name !== model.id && (
                    <div className="truncate font-mono text-caption text-text-faint">
                      {model.id}
                    </div>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  {model.input?.includes("image") && (
                    <span className="inline-flex shrink-0 items-center rounded-full border border-border bg-panel-2 px-1.5 py-0.5 text-caption text-text-dim">
                      {t("settings.badgeVision")}
                    </span>
                  )}
                  {model.context_window != null && <TokenBadge tokens={model.context_window} />}
                </div>
                <div className="flex shrink-0 items-center gap-0.5">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
                    onClick={() => {
                      void navigator.clipboard.writeText(model.id);
                      setCopiedModelId(model.id);
                      setTimeout(() => setCopiedModelId(null), 1500);
                    }}
                    aria-label={t("settings.copyModelId")}
                    title={
                      copiedModelId === model.id
                        ? t("settings.modelIdCopied")
                        : t("settings.copyModelId")
                    }
                  >
                    {copiedModelId === model.id ? (
                      <Check className="h-3.5 w-3.5 text-pass" />
                    ) : (
                      <Copy className="h-3.5 w-3.5" />
                    )}
                  </Button>
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
  );
}
