// The selected scenario's sections. Overview is the home; providers and
// plugins live in their own features. When there is no scenario at all (or
// the selection vanished), the creator takes over the whole view.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ScenarioSection } from "@/app/router";
import { useScenarioStore } from "@/app/store/scenarios";
import { useBusyStore } from "@/app/store/busy";
import type { Surface } from "@/shared/api/api";
import { Card, CardContent } from "@/shared/ui/card";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Label } from "@/shared/ui/label";
import { PageBody } from "@/shared/ui/page";
import { SurfacePicker } from "./new-scenario-dialog";
import { ScenarioOverview } from "@/features/runtime/scenario-overview";
import { ProvidersPage } from "@/features/providers/providers-page";
import { PluginsPage } from "@/features/plugins/plugins-page";

export function ScenarioSections({ section }: { section: ScenarioSection }) {
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const profile = useScenarioStore(
    (s) => s.profiles.find((p) => p.id === selectedScenario) ?? null,
  );

  if (!profile) {
    return <ScenarioCreator />;
  }
  if (section === "providers") {
    return <ProvidersPage key={profile.id} profile={profile} />;
  }
  if (section === "plugins") {
    return <PluginsPage key={profile.id} profile={profile} />;
  }
  return <ScenarioOverview key={profile.id} profile={profile} />;
}

function ScenarioCreator() {
  const { t } = useTranslation();
  const createScenario = useScenarioStore((s) => s.createScenario);
  const busy = useBusyStore((s) => s.busyAction) !== null;
  const [name, setName] = useState("");
  const [surface, setSurface] = useState<Surface>("web");

  const create = async () => {
    const trimmed = name.trim();
    if (!trimmed || busy) return;
    try {
      // The store selects the new scenario; this view re-renders into it.
      await createScenario(trimmed, surface);
    } catch {
      // The failure toast already surfaced.
    }
  };

  return (
    <PageBody className="space-y-5">
      <Card>
        <CardContent className="p-4 md:p-5">
          <div className="text-display font-bold text-text">{t("settings.newScenarioTitle")}</div>
          <p className="mt-1 text-small text-text-dim">{t("overview.noScenarios")}</p>
          <div className="mt-4 max-w-xl space-y-3">
            <div className="space-y-1">
              <Label>{t("settings.profileName")}</Label>
              <Input
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder={t("settings.profileName")}
                autoFocus
                onKeyDown={(event) => {
                  if (event.key === "Enter") create();
                }}
              />
            </div>
            <SurfacePicker value={surface} onChange={setSurface} />
            <Button variant="primary" onClick={create} disabled={busy || !name.trim()}>
              {t("settings.addProfile")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </PageBody>
  );
}
