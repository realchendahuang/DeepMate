// The per-scenario providers & models page: a full-height provider sidebar
// on the left and the selected provider's settings plus model list on the
// right.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, Pencil } from "lucide-react";
import { useProviderStore } from "@/app/store/providers";
import type { Model, Profile } from "@/shared/api/api";
import { Button } from "@/shared/ui/button";
import { Skeleton } from "@/shared/ui/skeleton";
import { EmptyState } from "@/shared/ui/empty-state";
import { PageBody, PageHeader } from "@/shared/ui/page";
import { ModelsPanel } from "./models-panel";
import { NewProviderDialog } from "./provider-dialog";
import { ModelDialog } from "./model-dialog";

export function ProvidersPage({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const loadProviders = useProviderStore((s) => s.loadProviders);
  const loadModels = useProviderStore((s) => s.loadModels);
  const removeProvider = useProviderStore((s) => s.removeProvider);
  const removeModel = useProviderStore((s) => s.removeModel);
  // Presence in the per-scenario maps doubles as the loaded flag.
  const providers = useProviderStore((s) => s.providers[profile.id]);
  const models = useProviderStore((s) => s.models[profile.id]);

  const [providerId, setProviderId] = useState<string | null>(null);
  const [modelDialog, setModelDialog] = useState<Model | "new" | null>(null);
  const [newProviderDialog, setNewProviderDialog] = useState(false);

  // Refresh on every visit; the cached slice renders instantly in between.
  useEffect(() => {
    void loadProviders(profile.id);
    void loadModels(profile.id);
  }, [loadProviders, loadModels, profile.id]);

  const loaded = providers !== undefined && models !== undefined;

  const selected =
    (providers ?? []).find((provider) => provider.id === providerId) ??
    (providers ?? [])[0] ??
    null;

  return (
    <PageBody full className="gap-4">
      <PageHeader title={t("nav.providers")} />
      {!loaded ? (
        <Skeleton className="min-h-0 flex-1" />
      ) : (providers ?? []).length === 0 ? (
        <EmptyState
          icon={<Pencil className="h-8 w-8 text-text-faint" />}
          action={
            <Button variant="secondary" size="sm" onClick={() => setNewProviderDialog(true)}>
              <Download className="h-4 w-4" />
              {t("settings.addProvider")}
            </Button>
          }
        >
          {t("settings.noProviders")}
        </EmptyState>
      ) : (
        <div className="min-h-0 flex-1">
          <ModelsPanel
            profile={profile.id}
            providers={providers ?? []}
            models={models ?? []}
            selectedId={selected?.id ?? null}
            onSelect={setProviderId}
            onNewProvider={() => setNewProviderDialog(true)}
            onDeleteProvider={(provider) => removeProvider(profile.id, provider.id)}
            onNewModel={() => setModelDialog("new")}
            onEditModel={(model) => setModelDialog(model)}
            onDeleteModel={(model) => removeModel(profile.id, model.provider ?? "", model.id)}
          />
        </div>
      )}

      <NewProviderDialog
        profile={profile.id}
        open={newProviderDialog}
        onClose={() => setNewProviderDialog(false)}
        onCreated={(id) => setProviderId(id)}
      />
      <ModelDialog
        key={
          modelDialog === "new" ? `new-${providerId ?? "none"}` : (modelDialog?.id ?? "closed")
        }
        profile={profile.id}
        open={modelDialog !== null}
        value={modelDialog === "new" ? null : modelDialog}
        defaultProvider={providerId ?? ""}
        onClose={() => setModelDialog(null)}
      />
    </PageBody>
  );
}
