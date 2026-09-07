// The mobile navigation drawer: the same SidebarNav body in a sheet, with
// the scenario/settings title as its header.

import { useTranslation } from "react-i18next";
import { useRouterStore } from "../router";
import { SidebarNav } from "./app-sidebar";
import { useScenarioStore } from "../store/scenarios";
import { Sheet, SheetContent, SheetTitle } from "@/shared/ui/sheet";

export function MobileNav({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const route = useRouterStore((s) => s.route);
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const scenarios = useScenarioStore((s) => s.profiles);

  const title =
    route.kind === "scenarios"
      ? t("settings.scenarios")
      : route.kind === "settings"
        ? t("settings.title")
        : (scenarios.find((profile) => profile.id === selectedScenario)?.name ??
          t("settings.title"));

  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent side="left" className="p-0" showCloseButton={false}>
        <SheetTitle className="sr-only">{t("nav.menu")}</SheetTitle>
        <div className="flex h-[52px] shrink-0 items-center gap-2 border-b border-border px-4">
          <span className="min-w-0 truncate text-heading font-semibold text-text">{title}</span>
        </div>
        {/* Any nav click closes the drawer. */}
        <div className="flex min-h-0 flex-1 flex-col" onClick={onClose}>
          <SidebarNav pillId="mobile-nav" />
        </div>
      </SheetContent>
    </Sheet>
  );
}
