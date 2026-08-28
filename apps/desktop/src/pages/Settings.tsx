import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Layers,
  Cloud,
  Cpu,
  Plus,
  Pencil,
  Trash2,
} from "lucide-react";
import { useStore } from "../store";
import type { Model, Provider } from "../types";
import { Card, CardContent } from "../components/ui/card";
import { Segmented } from "../components/ui/segmented";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { DataList, DataListRow, EmptyState } from "../components/ui/data-list";
import { PageBody, PageHeader, SectionHeader } from "../components/ui/page";
import { Dialog } from "../components/ui/dialog";
import { Switch } from "../components/ui/switch";

// A labelled form row used inside the edit dialogs.
function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1">
      <label className="block text-caption font-bold text-text-dim">{label}</label>
      {children}
      {hint && <p className="text-small text-text-faint">{hint}</p>}
    </div>
  );
}

export function SettingsPage() {
  const { t } = useTranslation();
  const [section, setSection] = useState(0); // 0 = configuration, 1 = preferences

  const profiles = useStore((s) => s.profiles);
  const providers = useStore((s) => s.providers);
  const models = useStore((s) => s.models);
  const loadProfiles = useStore((s) => s.loadProfiles);
  const loadProviders = useStore((s) => s.loadProviders);
  const loadModels = useStore((s) => s.loadModels);
  const setLanguage = useStore((s) => s.setLanguage);
  const setTheme = useStore((s) => s.setTheme);
  const setCloseToTray = useStore((s) => s.setCloseToTray);
  const setCheckUpdates = useStore((s) => s.setCheckUpdates);
  const setNotifyUpdates = useStore((s) => s.setNotifyUpdates);
  const setAutostart = useStore((s) => s.setAutostart);
  const checkUpdate = useStore((s) => s.checkUpdate);
  const openRelease = useStore((s) => s.openRelease);
  const configExport = useStore((s) => s.configExport);
  const configImport = useStore((s) => s.configImport);
  const language = useStore((s) => s.language);
  const theme = useStore((s) => s.theme);
  const closeToTray = useStore((s) => s.closeToTray);
  const checkUpdates = useStore((s) => s.checkUpdates);
  const notifyUpdates = useStore((s) => s.notifyUpdates);
  const autostart = useStore((s) => s.autostart);
  const updateInfo = useStore((s) => s.updateInfo);
  const updateChecked = useStore((s) => s.updateChecked);
  const busy = useStore((s) => s.busy);
  const snapshots = useStore((s) => s.snapshots);
  const loadSnapshots = useStore((s) => s.loadSnapshots);
  const snapshotExport = useStore((s) => s.snapshotExport);
  const snapshotImport = useStore((s) => s.snapshotImport);
  const { removeProfile, createProfile, removeProvider, removeModel } = useSettingsActions();

  // Edit-dialog state: which kind and which item (null id = create new).
  const [providerDialog, setProviderDialog] = useState<Provider | "new" | null>(null);
  const [modelDialog, setModelDialog] = useState<Model | "new" | null>(null);
  const [newProfile, setNewProfile] = useState("");
  const [showNewProfile, setShowNewProfile] = useState(false);
  const [snapshotName, setSnapshotName] = useState("");

  useEffect(() => {
    if (section === 0) {
      loadProfiles();
      loadProviders();
      loadModels();
      loadSnapshots();
    }
  }, [section, loadProfiles, loadProviders, loadModels, loadSnapshots]);

  const profileActions = (
    <>
      {showNewProfile && (
        <Input
          value={newProfile}
          onChange={(event) => setNewProfile(event.target.value)}
          placeholder={t("settings.profileName")}
          className="w-[180px]"
          onKeyDown={(event) => {
            if (event.key === "Enter" && newProfile.trim()) {
              createProfile(newProfile.trim());
              setNewProfile("");
              setShowNewProfile(false);
            }
            if (event.key === "Escape") setShowNewProfile(false);
          }}
        />
      )}
      <Button
        variant="secondary"
        size="sm"
        onClick={() => {
          if (showNewProfile && newProfile.trim()) {
            createProfile(newProfile.trim());
            setNewProfile("");
          }
          setShowNewProfile(!showNewProfile);
        }}
      >
        <Plus className="h-4 w-4" />
        {t("settings.addProfile")}
      </Button>
    </>
  );

  return (
    <PageBody className="space-y-5">
      <PageHeader title={t("settings.title")} />

      <Card>
        <CardContent className="p-2">
          <Segmented
            variant="full"
            options={[t("settings.configuration"), t("settings.preferences")]}
            value={section}
            onChange={setSection}
          />
        </CardContent>
      </Card>

      {section === 0 && (
        <div className="space-y-5">
          <section className="space-y-3">
            <SectionHeader title={t("settings.profiles")} actions={profileActions} />
            {profiles.length === 0 ? (
              <EmptyState icon={<Layers className="h-8 w-8 text-text-faint" />}>
                {t("settings.noProfiles")}
              </EmptyState>
            ) : (
              <Card className="overflow-hidden">
                <div className="divide-y divide-border">
                  {profiles.map((profile) => (
                    <div
                      key={profile.id}
                      className="flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover md:px-5"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-body font-semibold text-text">{profile.name}</div>
                        <div className="mt-0.5 truncate text-small text-text-faint lg:hidden">{profile.id}</div>
                      </div>
                      <div className="hidden shrink-0 text-small text-text-dim lg:block">{profile.id}</div>
                      <div className="hidden shrink-0 min-w-0 flex-1 truncate text-small text-text-faint lg:block">
                        {profile.description ?? ""}
                      </div>
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => removeProfile(profile.id)}
                        disabled={profile.id === "web"}
                        title={profile.id === "web" ? t("settings.protectedProfile") : undefined}
                      >
                        <Trash2 className="h-4 w-4" />
                        {t("settings.remove")}
                      </Button>
                    </div>
                  ))}
                </div>
              </Card>
            )}
          </section>

          <section className="space-y-3">
            <SectionHeader
              title={t("settings.providers")}
              actions={
                <Button variant="secondary" size="sm" onClick={() => setProviderDialog("new")}>
                  <Plus className="h-4 w-4" />
                  {t("settings.addProvider")}
                </Button>
              }
            />
            {providers.length === 0 ? (
              <EmptyState icon={<Cloud className="h-8 w-8 text-text-faint" />}>
                {t("settings.noProviders")}
              </EmptyState>
            ) : (
              <DataList
                columns={[
                  { label: t("settings.name") },
                  { label: t("settings.kind"), className: "w-[150px]" },
                  { label: "ID", compact: true },
                ]}
              >
                {providers.map((provider) => (
                  <DataListRow
                    key={provider.id}
                    primary={provider.name}
                    secondary={
                      <span className="flex items-center gap-1.5">
                        <span className="block max-w-[110px] truncate">{provider.kind}</span>
                        <Button variant="ghost" size="icon" onClick={() => setProviderDialog(provider)}>
                          <Pencil className="h-4 w-4" />
                        </Button>
                        {provider.id !== "deepseek-official" && (
                          <Button variant="ghost" size="icon" onClick={() => removeProvider(provider.id)}>
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        )}
                      </span>
                    }
                    detail={provider.id}
                  />
                ))}
              </DataList>
            )}
          </section>

          <section className="space-y-3">
            <SectionHeader
              title={t("settings.models")}
              actions={
                <Button variant="secondary" size="sm" onClick={() => setModelDialog("new")}>
                  <Plus className="h-4 w-4" />
                  {t("settings.addModel")}
                </Button>
              }
            />
            {models.length === 0 ? (
              <EmptyState icon={<Cpu className="h-8 w-8 text-text-faint" />}>
                {t("settings.noModels")}
              </EmptyState>
            ) : (
              <DataList
                columns={[
                  { label: t("settings.name") },
                  { label: t("settings.provider"), className: "w-[170px]" },
                  { label: "ID", compact: true },
                ]}
              >
                {models.map((model) => (
                  <DataListRow
                    key={`${model.provider ?? ""}/${model.id}`}
                    primary={model.name}
                    secondary={
                      <span className="flex items-center gap-1.5">
                        <span className="block max-w-[120px] truncate">{model.provider ?? "-"}</span>
                        <Button variant="ghost" size="icon" onClick={() => setModelDialog(model)}>
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => model.provider && removeModel(model.provider, model.id)}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </span>
                    }
                    detail={model.id}
                  />
                ))}
              </DataList>
            )}
          </section>

          <section className="space-y-3">
            <SectionHeader title={t("settings.snapshots")} />
            <Card>
              <CardContent className="flex flex-col gap-2 p-4 md:flex-row">
                <Input
                  value={snapshotName}
                  onChange={(event) => setSnapshotName(event.target.value)}
                  placeholder={t("settings.snapshotName")}
                  className="flex-1"
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && snapshotName.trim()) {
                      snapshotExport(snapshotName.trim());
                      setSnapshotName("");
                    }
                  }}
                />
                <Button
                  variant="primary"
                  onClick={() => {
                    if (snapshotName.trim()) {
                      snapshotExport(snapshotName.trim());
                      setSnapshotName("");
                    }
                  }}
                  disabled={!snapshotName.trim()}
                >
                  <Plus className="h-4 w-4" />
                  {t("settings.exportSnapshot")}
                </Button>
              </CardContent>
            </Card>

            {snapshots.length === 0 ? (
              <EmptyState icon={<Layers className="h-8 w-8 text-text-faint" />}>
                {t("settings.noSnapshots")}
              </EmptyState>
            ) : (
              <Card className="overflow-hidden">
                <div className="divide-y divide-border">
                  {snapshots.map((name) => (
                    <div
                      key={name}
                      className="flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover md:px-5"
                    >
                      <div className="min-w-0 flex-1 truncate text-body font-semibold text-text">
                        {name}
                      </div>
                      <Button variant="secondary" size="sm" onClick={() => snapshotImport(name)}>
                        {t("settings.importSnapshot")}
                      </Button>
                    </div>
                  ))}
                </div>
              </Card>
            )}
          </section>
        </div>
      )}

      {section === 1 && (
        <section className="space-y-3">
          <SectionHeader title={t("settings.preferences")} />
          <Card>
            <div className="divide-y divide-border">
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.language")}</div>
                  <div className="text-small text-text-dim">{t("settings.general")}</div>
                </div>
                <Segmented
                  options={["English", "简体中文"]}
                  value={language === "zh" ? 1 : 0}
                  onChange={(index) => setLanguage(index === 0 ? "en" : "zh")}
                />
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.theme")}</div>
                  <div className="text-small text-text-dim">{t("settings.appearance")}</div>
                </div>
                <Segmented
                  options={["System", "Light", "Dark"]}
                  value={Math.max(0, ["system", "light", "dark"].indexOf(theme))}
                  onChange={(index) => setTheme(["system", "light", "dark"][index])}
                />
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.autoStart")}</div>
                  <div className="text-small text-text-dim">{t("settings.autoStartHint")}</div>
                </div>
                <Switch
                  checked={autostart}
                  onChange={setAutostart}
                  aria-label={t("settings.autoStart")}
                />
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.closeToTray")}</div>
                  <div className="text-small text-text-dim">{t("settings.closeToTrayHint")}</div>
                </div>
                <Switch
                  checked={closeToTray}
                  onChange={setCloseToTray}
                  aria-label={t("settings.closeToTray")}
                />
              </div>
              <div className="space-y-3 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-heading font-semibold text-text">{t("settings.updates")}</div>
                    <div className="text-small text-text-dim">{t("settings.updatesHint")}</div>
                  </div>
                  <Switch
                    checked={checkUpdates}
                    onChange={setCheckUpdates}
                    aria-label={t("settings.updates")}
                  />
                </div>
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="text-heading font-semibold text-text">{t("settings.notifications")}</div>
                    <div className="text-small text-text-dim">{t("settings.notificationsHint")}</div>
                  </div>
                  <Switch
                    checked={notifyUpdates}
                    onChange={setNotifyUpdates}
                    aria-label={t("settings.notifications")}
                  />
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <Button variant="secondary" size="sm" onClick={checkUpdate} disabled={busy}>
                    {t("settings.checkNow")}
                  </Button>
                  {updateInfo && (
                    <Button
                      variant="primary"
                      size="sm"
                      onClick={() => openRelease(updateInfo.url)}
                      title={updateInfo.url}
                    >
                      {t("settings.updateAvailable", { version: updateInfo.latest_version })}
                    </Button>
                  )}
                  {updateChecked && !updateInfo && (
                    <span className="text-small text-text-dim">{t("settings.upToDate")}</span>
                  )}
                </div>
              </div>
              <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-heading font-semibold text-text">{t("settings.ownSettings")}</div>
                  <div className="text-small text-text-dim">{t("settings.ownSettingsHint")}</div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button variant="secondary" size="sm" onClick={configExport} disabled={busy}>
                    {t("settings.exportSettings")}
                  </Button>
                  <Button variant="secondary" size="sm" onClick={configImport} disabled={busy}>
                    {t("settings.importSettings")}
                  </Button>
                </div>
              </div>
            </div>
          </Card>
        </section>
      )}

      <ProviderDialog
        open={providerDialog !== null}
        value={providerDialog === "new" ? null : providerDialog}
        onClose={() => setProviderDialog(null)}
      />
      <ModelDialog
        open={modelDialog !== null}
        value={modelDialog === "new" ? null : modelDialog}
        onClose={() => setModelDialog(null)}
      />
    </PageBody>
  );
}

// ---- Provider editor ----

function ProviderDialog({
  open,
  value,
  onClose,
}: {
  open: boolean;
  value: Provider | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const upsertProvider = useStore((s) => s.upsertProvider);

  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [api, setApi] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKeyEnv, setApiKeyEnv] = useState("");
  const [compat, setCompat] = useState("");

  // Seed the form whenever the dialog opens for a different target.
  useEffect(() => {
    if (!open) return;
    setId(value?.id ?? "");
    setName(value?.name ?? "");
    setApi(value?.api ?? "");
    setBaseUrl(value?.base_url ?? "");
    setApiKeyEnv(value?.api_key_env ?? "");
    setCompat(value?.compat ?? "");
  }, [open, value]);

  const canSave = id.trim().length > 0 && name.trim().length > 0;
  const isDeepseek = value?.id === "deepseek-official";

  const save = async () => {
    if (!canSave) return;
    await upsertProvider({
      id: id.trim(),
      name: name.trim(),
      kind: id.trim() === "deepseek-official" ? "deepseek" : "pi-ai",
      api: api.trim() || null,
      base_url: baseUrl.trim() || null,
      api_key_env: apiKeyEnv.trim() || null,
      compat: compat.trim() || null,
    });
    onClose();
  };

  return (
    <Dialog open={open} onClose={onClose} title={value ? t("settings.editProvider") : t("settings.addProvider")}>
      <Field label={t("settings.id")} hint={isDeepseek ? t("settings.deepseekRoute") : undefined}>
        <Input value={id} onChange={(event) => setId(event.target.value)} disabled={isDeepseek} placeholder="openai" />
      </Field>
      <Field label={t("settings.name")}>
        <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="OpenAI" />
      </Field>
      <Field label={t("settings.api")}>
        <Input value={api} onChange={(event) => setApi(event.target.value)} placeholder="openai-responses" />
      </Field>
      <Field label={t("settings.baseUrl")}>
        <Input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder="https://api.example.com/v1" />
      </Field>
      <Field label={t("settings.apiKeyEnv")} hint={t("settings.apiKeyEnvHint")}>
        <Input value={apiKeyEnv} onChange={(event) => setApiKeyEnv(event.target.value)} placeholder="MY_API_KEY" />
      </Field>
      <Field label={t("settings.compat")} hint={t("settings.compatHint")}>
        <textarea
          value={compat}
          onChange={(event) => setCompat(event.target.value)}
          className="h-[72px] w-full rounded-md border border-border bg-inset px-2.5 py-1.5 text-body text-text placeholder:text-text-faint focus:outline-none focus:border-accent focus:ring-2 focus:ring-accent/20"
          placeholder='{"supportsStore":false}'
        />
      </Field>
      <div className="flex justify-end gap-2 pt-1">
        <Button variant="ghost" onClick={onClose}>{t("settings.cancel")}</Button>
        <Button variant="primary" onClick={save} disabled={!canSave}>{t("settings.save")}</Button>
      </div>
    </Dialog>
  );
}

// ---- Model editor ----

function ModelDialog({
  open,
  value,
  onClose,
}: {
  open: boolean;
  value: Model | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const upsertModel = useStore((s) => s.upsertModel);

  const [provider, setProvider] = useState("");
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [contextWindow, setContextWindow] = useState("");
  const [maxTokens, setMaxTokens] = useState("");
  const [input, setInput] = useState("");
  const [reasoning, setReasoning] = useState("");
  const [compat, setCompat] = useState("");

  useEffect(() => {
    if (!open) return;
    setProvider(value?.provider ?? providers[0]?.id ?? "");
    setId(value?.id ?? "");
    setName(value?.name ?? "");
    setContextWindow(value?.context_window ? String(value.context_window) : "");
    setMaxTokens(value?.max_tokens ? String(value.max_tokens) : "");
    setInput(value?.input?.join(",") ?? "");
    setReasoning(value?.reasoning_efforts ?? "");
    setCompat(value?.compat ?? "");
  }, [open, value, providers]);

  const canSave = provider.trim().length > 0 && id.trim().length > 0 && name.trim().length > 0;

  const save = async () => {
    if (!canSave) return;
    await upsertModel(provider.trim(), {
      id: id.trim(),
      name: name.trim(),
      provider: provider.trim(),
      context_window: contextWindow.trim() ? Number(contextWindow.trim()) : null,
      max_tokens: maxTokens.trim() ? Number(maxTokens.trim()) : null,
      input: input.trim()
        ? input.split(",").map((part) => part.trim()).filter((part) => part.length > 0)
        : null,
      reasoning_efforts: reasoning.trim() || null,
      compat: compat.trim() || null,
    });
    onClose();
  };

  return (
    <Dialog open={open} onClose={onClose} title={value ? t("settings.editModel") : t("settings.addModel")}>
      <Field label={t("settings.provider")}>
        <select
          value={provider}
          onChange={(event) => setProvider(event.target.value)}
          className="h-[32px] w-full rounded-md border border-border bg-inset px-2.5 text-body text-text focus:outline-none focus:border-accent"
        >
          {providers.map((providerOption) => (
            <option key={providerOption.id} value={providerOption.id}>
              {providerOption.name} ({providerOption.id})
            </option>
          ))}
        </select>
      </Field>
      <Field label={t("settings.id")}>
        <Input value={id} onChange={(event) => setId(event.target.value)} placeholder="gpt-4o" />
      </Field>
      <Field label={t("settings.name")}>
        <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="GPT-4o" />
      </Field>
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("settings.contextWindow")}>
          <Input value={contextWindow} onChange={(event) => setContextWindow(event.target.value)} placeholder="262144" inputMode="numeric" />
        </Field>
        <Field label={t("settings.maxTokens")}>
          <Input value={maxTokens} onChange={(event) => setMaxTokens(event.target.value)} placeholder="32768" inputMode="numeric" />
        </Field>
      </div>
      <Field label={t("settings.input")} hint={t("settings.inputHint")}>
        <Input value={input} onChange={(event) => setInput(event.target.value)} placeholder="text,image" />
      </Field>
      <Field label={t("settings.reasoningEfforts")} hint={t("settings.compatHint")}>
        <textarea
          value={reasoning}
          onChange={(event) => setReasoning(event.target.value)}
          className="h-[56px] w-full rounded-md border border-border bg-inset px-2.5 py-1.5 text-body text-text placeholder:text-text-faint focus:outline-none focus:border-accent focus:ring-2 focus:ring-accent/20"
          placeholder='{"max":"max"}'
        />
      </Field>
      <Field label={t("settings.compat")} hint={t("settings.compatHint")}>
        <textarea
          value={compat}
          onChange={(event) => setCompat(event.target.value)}
          className="h-[56px] w-full rounded-md border border-border bg-inset px-2.5 py-1.5 text-body text-text placeholder:text-text-faint focus:outline-none focus:border-accent focus:ring-2 focus:ring-accent/20"
          placeholder='{"supportsStore":false}'
        />
      </Field>
      <div className="flex justify-end gap-2 pt-1">
        <Button variant="ghost" onClick={onClose}>{t("settings.cancel")}</Button>
        <Button variant="primary" onClick={save} disabled={!canSave}>{t("settings.save")}</Button>
      </div>
    </Dialog>
  );
}

// Wired store accessors so the page reads from one source of truth. Kept at
// the bottom to keep the render functions focused on layout.
function useSettingsActions() {
  const removeProfile = useStore((s) => s.removeProfile);
  const createProfile = useStore((s) => s.createProfile);
  const removeProvider = useStore((s) => s.removeProvider);
  const removeModel = useStore((s) => s.removeModel);
  return { removeProfile, createProfile, removeProvider, removeModel };
}