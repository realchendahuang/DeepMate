// Delete a scenario. The scenario store handles the aftermath: when the
// deleted scenario was selected, the selection falls back to the first
// remaining one.

import { useTranslation } from "react-i18next";
import { useScenarioStore } from "@/app/store/scenarios";
import type { Profile } from "@/shared/api/api";
import { ConfirmDialog } from "@/shared/ui/confirm-dialog";

export function ScenarioDeleteDialog({
  profile,
  open,
  onOpenChange,
}: {
  profile: Profile | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const removeScenario = useScenarioStore((s) => s.removeScenario);

  return (
    <ConfirmDialog
      open={open && profile !== null}
      onOpenChange={onOpenChange}
      title={t("settings.deleteProfileConfirmTitle")}
      body={t("settings.deleteProfileConfirmBody", { name: profile?.name ?? "" })}
      onConfirm={() => {
        if (profile) removeScenario(profile.id);
        onOpenChange(false);
      }}
    />
  );
}
