import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Layers,
  Cloud,
  Cpu,
  Plus,
  Pencil,
  Trash2,
  Camera,
} from "lucide-react";
import { useStore } from "../store";
import type { Model, Provider } from "../api";
import { Card, CardContent } from "../components/ui/card";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { Label } from "../components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { Skeleton } from "../components/ui/skeleton";
import { EmptyState } from "../components/ui/empty-state";
import { ConfirmDialog } from "../components/ui/confirm-dialog";
import { PageBody, PageHeader, SectionHeader } from "../components/ui/page";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "../components/ui/dialog";
import { Switch } from "../components/ui/switch";
import { cn } from "../lib/utils";

// Settings sections, in display order. The sub-nav (desktop) and the chip
// strip (mobile) both render from this list.
const SECTIONS = [
  { key: "profiles", icon: Layers },
  { key: "providers", icon: Cloud },
  { key: "models", icon: Cpu },
  { key: "snapshots", icon: Camera },
  { key: "preferences", icon: null },
] as const;

type SectionKey = (typeof SECTIONS)[number]["key"];

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
      <Label>{label}</Label>
      {children}
      {hint && <p className="text-small text-text-faint">{hint}</p>}
    </div>
  );
}

export function SettingsPage() {
  const { t } = useTranslation();
  const [section, setSection] = useState<SectionKey>("profiles");

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
  const installUpdate = useStore((s) => s.installUpdate);
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
  const busyAction = useStore((s) => s.busyAction);
  const snapshots = useStore((s) => s.snapshots);
  const loadSnapshots = useStore((s) => s.loadSnapshots);
  const snapshotExport = useStore((s) => s.snapshotExport);
  const snapshotImport = useStore((s) => s.snapshotImport);
  const snapshotDelete = useStore((s) => s.snapshotDelete);
  const { removeProfile, createProfile, removeProvider, removeModel } = useSettingsActions();

  // Edit-dialog state: which kind and which item (null id = create new).
  const [providerDialog, setProviderDialog] = useState<Provider | "new" | null>(null);
  const [modelDialog, setModelDialog] = useState<Model | "new" | null>(null);
  const [newProfile, setNewProfile] = useState("");
  const [showNewProfile, setShowNewProfile] = useState(false);
  const [snapshotName, setSnapshotName] = useState("");

  // Confirm-dialog state: which item is pending deletion/import.
  const [confirm, setConfirm] = useState<{
    kind: "profile" | "provider" | "model" | "snapshot" | "snapshot-import" | "config-import";
    id: string;
    name: string;
  } | null>(null);

  // First-load tracking for skeletons.
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    Promise.all([loadProfiles(), loadProviders(), loadModels(), loadSnapshots()]).then(() =>
      setLoaded(true),
    );
  }, [loadProfiles, loadProviders, loadModels, loadSnapshots]);

  const busy = busyAction !== null;

  const runConfirm = () => {
    if (!confirm) return;
    const { kind, id, name } = confirm;
    setConfirm(null);
    switch (kind) {
      case "profile":
        removeProfile(id);
        break;
      case "provider":
        removeProvider(id);
        break;
      case "model": {
        const [provider, modelId] = id.split("/");
        if (provider) removeModel(provider, modelId);
        break;
      }
      case "snapshot":
        snapshotDelete(name);
        break;
      case "snapshot-import":
        snapshotImport(name);
        break;
      case "config-import":
        configImport();
        break;
    }
  };

  const confirmTitle = confirm
    ? t(`settings.${confirm.kind === "snapshot-import" ? "importSnapshot" : confirm.kind === "config-import" ? "importConfig" : "delete"}ConfirmTitle`)
    : "";
  const confirmBody = confirm
    ? t(
        `settings.${
          confirm.kind === "snapshot-import"
            ? "importSnapshot"
            : confirm.kind === "config-import"
              ? "importConfig"
              : "delete"
        }ConfirmBody`,
        { name: confirm.name },
      )
    : "";

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
        <CardContent className="flex flex-col gap-4 p-4 md:flex-row md:gap-0 md:p-0">
          {/* Section sub-nav: vertical on desktop, a horizontal chip strip on
              mobile. */}
          <nav
            aria-label={t("settings.title")}
            className="flex shrink-0 gap-1 overflow-x-auto pb-1 md:w-subnav md:flex-col md:gap-0.5 md:overflow-visible md:border-r md:border-border md:p-2 md:pb-2"
          >
            {SECTIONS.map(({ key, icon: Icon }) => (
              <button
                key={key}
                type="button"
                onClick={() => setSection(key)}
                aria-current={section === key ? "true" : undefined}
                className={cn(
                  "flex h-8 shrink-0 items-center gap-2 rounded-md px-2.5 text-body font-medium whitespace-nowrap transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
                  section === key
                    ? "bg-accent-soft text-accent"
                    : "text-text-dim hover:bg-hover hover:text-text",
                )}
              >
                {Icon && <Icon className="h-4 w-4 shrink-0" />}
                <span className="truncate">{t(`settings.${key}`)}</span>
              </button>
            ))}
          </nav>

          {/* Section content. */}
          <div className="min-w-0 flex-1 space-y-5 md:p-5">
            {section === "profiles" && (
              <section className="space-y-3">
                <SectionHeader title={t("settings.profiles")} actions={profileActions} />
                {!loaded ? (
                  <Skeleton className="h-24 w-full" />
                ) : profiles.length === 0 ? (
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
                          <div className="hidden min-w-0 flex-1 truncate text-small text-text-faint lg:block">
                            {profile.description ?? ""}
                          </div>
                          <Button
                            variant="danger"
                            size="sm"
                            onClick={() =>
                              setConfirm({ kind: "profile", id: profile.id, name: profile.name })
                            }
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
            )}

            {section === "providers" && (
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
                {!loaded ? (
                  <Skeleton className="h-24 w-full" />
                ) : providers.length === 0 ? (
                  <EmptyState icon={<Cloud className="h-8 w-8 text-text-faint" />}>
                    {t("settings.noProviders")}
                  </EmptyState>
                ) : (
                  <Card className="overflow-hidden">
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead>{t("settings.name")}</TableHead>
                          <TableHead className="w-[150px]">{t("settings.kind")}</TableHead>
                          <TableHead className="hidden lg:table-cell">ID</TableHead>
                          <TableHead className="w-[80px]" />
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {providers.map((provider) => (
                          <TableRow key={provider.id}>
                            <TableCell className="font-semibold text-text">{provider.name}</TableCell>
                            <TableCell className="text-text-dim">{provider.kind}</TableCell>
                            <TableCell className="hidden text-text-faint lg:table-cell">
                              {provider.id}
                            </TableCell>
                            <TableCell>
                              <div className="flex items-center gap-1">
                                <Button variant="ghost" size="icon" onClick={() => setProviderDialog(provider)}>
                                  <Pencil className="h-4 w-4" />
                                </Button>
                                {provider.id !== "deepseek-official" && (
                                  <Button
                                    variant="ghost"
                                    size="icon"
                                    onClick={() =>
                                      setConfirm({ kind: "provider", id: provider.id, name: provider.name })
                                    }
                                  >
                                    <Trash2 className="h-4 w-4" />
                                  </Button>
                                )}
                              </div>
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </Card>
                )}
              </section>
            )}

            {section === "models" && (
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
                {!loaded ? (
                  <Skeleton className="h-24 w-full" />
                ) : models.length === 0 ? (
                  <EmptyState icon={<Cpu className="h-8 w-8 text-text-faint" />}>
                    {t("settings.noModels")}
                  </EmptyState>
                ) : (
                  <Card className="overflow-hidden">
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead>{t("settings.name")}</TableHead>
                          <TableHead className="w-[170px]">{t("settings.provider")}</TableHead>
                          <TableHead className="hidden lg:table-cell">ID</TableHead>
                          <TableHead className="w-[80px]" />
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {models.map((model) => (
                          <TableRow key={`${model.provider ?? ""}/${model.id}`}>
                            <TableCell className="font-semibold text-text">{model.name}</TableCell>
                            <TableCell className="text-text-dim">{model.provider ?? "-"}</TableCell>
                            <TableCell className="hidden text-text-faint lg:table-cell">
                              {model.id}
                            </TableCell>
                            <TableCell>
                              <div className="flex items-center gap-1">
                                <Button variant="ghost" size="icon" onClick={() => setModelDialog(model)}>
                                  <Pencil className="h-4 w-4" />
                                </Button>
                                <Button
                                  variant="ghost"
                                  size="icon"
                                  onClick={() =>
                                    model.provider &&
                                    setConfirm({
                                      kind: "model",
                                      id: `${model.provider}/${model.id}`,
                                      name: model.name,
                                    })
                                  }
                                >
                                  <Trash2 className="h-4 w-4" />
                                </Button>
                              </div>
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </Card>
                )}
              </section>
            )}

            {section === "snapshots" && (
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

                {!loaded ? (
                  <Skeleton className="h-24 w-full" />
                ) : snapshots.length === 0 ? (
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
                          <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => setConfirm({ kind: "snapshot-import", id: name, name })}
                          >
                            {t("settings.importSnapshot")}
                          </Button>
                          <Button
                            variant="danger"
                            size="sm"
                            onClick={() => setConfirm({ kind: "snapshot", id: name, name })}
                          >
                            <Trash2 className="h-4 w-4" />
                            {t("settings.deleteSnapshot")}
                          </Button>
                        </div>
                      ))}
                    </div>
                  </Card>
                )}
              </section>
            )}

            {section === "preferences" && (
              <section className="space-y-3">
                <SectionHeader title={t("settings.preferences")} />
                <Card>
                  <div className="divide-y divide-border">
                    <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                      <div>
                        <div className="text-heading font-semibold text-text">{t("settings.language")}</div>
                        <div className="text-small text-text-dim">{t("settings.general")}</div>
                      </div>
                      <Select value={language} onValueChange={(value) => setLanguage(value)}>
                        <SelectTrigger className="w-[140px]">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="en">English</SelectItem>
                          <SelectItem value="zh">简体中文</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                      <div>
                        <div className="text-heading font-semibold text-text">{t("settings.theme")}</div>
                        <div className="text-small text-text-dim">{t("settings.appearance")}</div>
                      </div>
                      <Select value={theme} onValueChange={(value) => setTheme(value)}>
                        <SelectTrigger className="w-[140px]">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="system">System</SelectItem>
                          <SelectItem value="light">Light</SelectItem>
                          <SelectItem value="dark">Dark</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                    <div className="flex flex-col gap-3 p-4 md:flex-row md:items-center md:justify-between">
                      <div>
                        <div className="text-heading font-semibold text-text">{t("settings.autoStart")}</div>
                        <div className="text-small text-text-dim">{t("settings.autoStartHint")}</div>
                      </div>
                      <Switch
                        checked={autostart}
                        onCheckedChange={setAutostart}
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
                        onCheckedChange={setCloseToTray}
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
                          onCheckedChange={setCheckUpdates}
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
                          onCheckedChange={setNotifyUpdates}
                          aria-label={t("settings.notifications")}
                        />
                      </div>
                      <div className="flex flex-wrap items-center gap-2">
                        <Button variant="secondary" size="sm" onClick={checkUpdate} disabled={busy}>
                          {t("settings.checkNow")}
                        </Button>
                        {updateInfo && (
                          <>
                            <Button variant="primary" size="sm" onClick={installUpdate} disabled={busy}>
                              {t("settings.installUpdate")}
                            </Button>
                            <Button
                              variant="primary"
                              size="sm"
                              onClick={() => openRelease(updateInfo.url)}
                              title={updateInfo.url}
                            >
                              {t("settings.updateAvailable", { version: updateInfo.latest_version })}
                            </Button>
                          </>
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
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={() => setConfirm({ kind: "config-import", id: "", name: "" })}
                          disabled={busy}
                        >
                          {t("settings.importSettings")}
                        </Button>
                      </div>
                    </div>
                  </div>
                </Card>
              </section>
            )}
          </div>
        </CardContent>
      </Card>

      <ProviderDialog
        key={providerDialog === "new" ? "new" : providerDialog?.id ?? "closed"}
        open={providerDialog !== null}
        value={providerDialog === "new" ? null : providerDialog}
        onClose={() => setProviderDialog(null)}
      />
      <ModelDialog
        key={modelDialog === "new" ? "new" : modelDialog?.id ?? "closed"}
        open={modelDialog !== null}
        value={modelDialog === "new" ? null : modelDialog}
        onClose={() => setModelDialog(null)}
      />

      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => !open && setConfirm(null)}
        title={confirmTitle}
        body={confirmBody}
        onConfirm={runConfirm}
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

  // The form is seeded from the target on mount; the caller remounts it per
  // target via the `key` prop, so no effect is needed.
  const [id, setId] = useState(value?.id ?? "");
  const [name, setName] = useState(value?.name ?? "");
  const [api, setApi] = useState(value?.api ?? "");
  const [baseUrl, setBaseUrl] = useState(value?.base_url ?? "");
  const [apiKeyEnv, setApiKeyEnv] = useState(value?.api_key_env ?? "");
  const [compat, setCompat] = useState(value?.compat ?? "");

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
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{value ? t("settings.editProvider") : t("settings.addProvider")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
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
            <Textarea
              value={compat}
              onChange={(event) => setCompat(event.target.value)}
              className="h-[72px]"
              placeholder='{"supportsStore":false}'
            />
          </Field>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>{t("settings.cancel")}</Button>
          <Button variant="primary" onClick={save} disabled={!canSave}>{t("settings.save")}</Button>
        </DialogFooter>
      </DialogContent>
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

  // The form is seeded from the target on mount; the caller remounts it per
  // target via the `key` prop, so no effect is needed.
  const [provider, setProvider] = useState(value?.provider ?? providers[0]?.id ?? "");
  const [id, setId] = useState(value?.id ?? "");
  const [name, setName] = useState(value?.name ?? "");
  const [contextWindow, setContextWindow] = useState(
    value?.context_window ? String(value.context_window) : "",
  );
  const [maxTokens, setMaxTokens] = useState(value?.max_tokens ? String(value.max_tokens) : "");
  const [input, setInput] = useState(value?.input?.join(",") ?? "");
  const [reasoning, setReasoning] = useState(value?.reasoning_efforts ?? "");
  const [compat, setCompat] = useState(value?.compat ?? "");

  const canSave = provider.trim().length > 0 && id.trim().length > 0 && name.trim().length > 0;

  // Numeric fields accept digits only.
  const digitsOnly = (value: string) => value.replace(/\D/g, "");

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
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{value ? t("settings.editModel") : t("settings.addModel")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <Field label={t("settings.provider")}>
            <Select value={provider} onValueChange={setProvider}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {providers.map((providerOption) => (
                  <SelectItem key={providerOption.id} value={providerOption.id}>
                    {providerOption.name} ({providerOption.id})
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field label={t("settings.id")}>
            <Input value={id} onChange={(event) => setId(event.target.value)} placeholder="gpt-4o" />
          </Field>
          <Field label={t("settings.name")}>
            <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="GPT-4o" />
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
          <Field label={t("settings.input")} hint={t("settings.inputHint")}>
            <Input value={input} onChange={(event) => setInput(event.target.value)} placeholder="text,image" />
          </Field>
          <Field label={t("settings.reasoningEfforts")} hint={t("settings.compatHint")}>
            <Textarea
              value={reasoning}
              onChange={(event) => setReasoning(event.target.value)}
              className="h-[56px]"
              placeholder='{"max":"max"}'
            />
          </Field>
          <Field label={t("settings.compat")} hint={t("settings.compatHint")}>
            <Textarea
              value={compat}
              onChange={(event) => setCompat(event.target.value)}
              className="h-[56px]"
              placeholder='{"supportsStore":false}'
            />
          </Field>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>{t("settings.cancel")}</Button>
          <Button variant="primary" onClick={save} disabled={!canSave}>{t("settings.save")}</Button>
        </DialogFooter>
      </DialogContent>
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
