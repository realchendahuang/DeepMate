// Edit a scenario's description (a free-text note shown under the scenario
// name). The engine's `bundles: ...` fallback string is not user-authored, so
// it is not offered as an initial draft; saving an empty text clears the
// description and restores that fallback.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useScenarioStore } from "@/app/store/scenarios";
import type { Profile } from "@/shared/api/api";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Textarea } from "@/shared/ui/textarea";

// The engine-bundled fallback description ("bundles: a, b, c") is derived,
// not user-authored; treat it as empty so the editor starts clean.
const BUNDLES_FALLBACK_PREFIX = "bundles:";

function initialDraft(description: string | null): string {
  if (!description?.trim()) return "";
  return description.startsWith(BUNDLES_FALLBACK_PREFIX) ? "" : description;
}

export function ScenarioDescriptionDialog({
  profile,
  open,
  onOpenChange,
}: {
  profile: Profile | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const updateDescription = useScenarioStore((s) => s.updateDescription);
  // The dialog content mounts fresh on every open, so the draft reseeds
  // from the profile without an effect.
  const [text, setText] = useState(() => initialDraft(profile?.description ?? null));

  const doSave = async () => {
    if (!profile) return;
    await updateDescription(profile.id, text);
    onOpenChange(false);
  };

  return (
    <Dialog open={open && profile !== null} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("settings.editDescriptionTitle")}</DialogTitle>
        </DialogHeader>
        <Textarea
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder={t("settings.descriptionPlaceholder")}
          rows={4}
          autoFocus
        />
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("settings.cancel")}
          </Button>
          <Button variant="primary" onClick={doSave}>
            {t("settings.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}