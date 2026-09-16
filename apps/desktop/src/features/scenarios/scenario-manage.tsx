// The shared "manage a scenario" surface: one hook for the dialog state plus
// the busy/protection lock, the context-menu body for the rail, and the
// mounted dialogs. The rail's context menu and the overview hero's icon
// buttons enforce the same rules (the default scenario cannot be renamed or
// removed; nothing may mutate while a busy action runs) because they go
// through this module instead of restating them.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, SquarePen, Trash2 } from "lucide-react";
import { useBlocking } from "@/app/store/busy";
import type { Profile } from "@/shared/api/api";
import { isDefaultScenario } from "@/shared/lib/scenario";
import { ContextMenuItem, ContextMenuSeparator } from "@/shared/ui/context-menu";
import { ScenarioRenameDialog } from "./scenario-rename-dialog";
import { ScenarioDescriptionDialog } from "./scenario-description-dialog";
import { ScenarioDeleteDialog } from "./scenario-delete-dialog";

export type ScenarioManageDialog = "rename" | "description" | "delete";

export function useScenarioManage() {
  const busy = useBlocking();
  const [target, setTarget] = useState<Profile | null>(null);
  const [dialog, setDialog] = useState<ScenarioManageDialog | null>(null);

  const open = (kind: ScenarioManageDialog, profile: Profile) => {
    setTarget(profile);
    setDialog(kind);
  };
  const close = () => setDialog(null);

  return { busy, target, dialog, open, close };
}

// The context-menu body shown when right-clicking a rail avatar.
export function ScenarioManageMenu({
  profile,
  busy,
  open,
}: {
  profile: Profile;
  busy: boolean;
  open: (kind: ScenarioManageDialog, profile: Profile) => void;
}) {
  const { t } = useTranslation();
  const isDefault = isDefaultScenario(profile);
  const locked = isDefault || busy;
  return (
    <>
      <ContextMenuItem onClick={() => open("description", profile)}>
        <FileText className="h-4 w-4 text-text-faint" />
        {t("settings.editDescription")}
      </ContextMenuItem>
      <ContextMenuItem disabled={locked} onClick={() => open("rename", profile)}>
        <SquarePen className="h-4 w-4 text-text-faint" />
        {isDefault ? t("settings.protectedProfile") : t("settings.rename")}
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem destructive disabled={locked} onClick={() => open("delete", profile)}>
        <Trash2 className="h-4 w-4" />
        {isDefault ? t("settings.protectedProfile") : t("settings.remove")}
      </ContextMenuItem>
    </>
  );
}

// The three dialogs mounted once per surface, driven by the hook's state.
export function ScenarioManageDialogs({
  manage,
}: {
  manage: ReturnType<typeof useScenarioManage>;
}) {
  return (
    <>
      <ScenarioRenameDialog
        profile={manage.target}
        open={manage.dialog === "rename" && manage.target !== null}
        onOpenChange={(openState) => !openState && manage.close()}
      />
      <ScenarioDescriptionDialog
        profile={manage.target}
        open={manage.dialog === "description" && manage.target !== null}
        onOpenChange={(openState) => !openState && manage.close()}
      />
      <ScenarioDeleteDialog
        profile={manage.target}
        open={manage.dialog === "delete" && manage.target !== null}
        onOpenChange={(openState) => !openState && manage.close()}
      />
    </>
  );
}
