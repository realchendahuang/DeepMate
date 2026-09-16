// Rename a scenario. The harness moves the profile directory, so the id
// changes; the scenario store moves the selection with it.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useScenarioStore } from "@/app/store/scenarios";
import type { Profile } from "@/shared/api/api";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/shared/ui/dialog";

export function ScenarioRenameDialog({
  profile,
  open,
  onOpenChange,
}: {
  profile: Profile | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const renameScenario = useScenarioStore((s) => s.renameScenario);
  // The dialog content mounts fresh on every open, so the draft reseeds
  // from the profile without an effect.
  const [name, setName] = useState(profile?.name ?? "");

  const doRename = () => {
    if (!profile) return;
    const trimmed = name.trim();
    if (!trimmed || trimmed === profile.name) return;
    renameScenario(profile.id, trimmed);
    onOpenChange(false);
  };

  return (
    <Dialog open={open && profile !== null} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.renameScenarioTitle")}</DialogTitle>
        </DialogHeader>
        <Input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={t("settings.profileName")}
          autoFocus
          onKeyDown={(event) => {
            if (event.key === "Enter") doRename();
            if (event.key === "Escape") onOpenChange(false);
          }}
        />
        <DialogFooter>
          <Button variant="secondary" onClick={() => onOpenChange(false)}>
            {t("settings.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={!name.trim() || name.trim() === profile?.name}
            onClick={doRename}
          >
            {t("settings.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
