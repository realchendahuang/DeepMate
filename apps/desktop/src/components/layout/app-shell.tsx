import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Menu, Cpu } from "lucide-react";
import { useStore } from "../../store";
import { Button } from "../ui/button";
import { TooltipProvider } from "../ui/tooltip";
import type { View } from "./nav";
import { AppSidebar } from "./app-sidebar";
import { MobileNav } from "./mobile-nav";
import { OverviewPage } from "../../pages/Overview";
import { PluginsPage } from "../../pages/Plugins";
import { SettingsPage } from "../../pages/Settings";

export function AppShell() {
  const { t } = useTranslation();
  const [view, setView] = useState<View>("overview");
  const [navOpen, setNavOpen] = useState(false);

  const harnessName = useStore((s) => s.overview?.detection.harness?.name ?? "");

  const navigate = (next: View) => {
    setView(next);
    setNavOpen(false);
  };

  return (
    <TooltipProvider>
      <div className="flex h-screen bg-bg text-text">
        {/* Desktop navigation. */}
        <AppSidebar view={view} onNavigate={navigate} harnessName={harnessName} />

        <div className="flex min-w-0 flex-1 flex-col">
          {/* Compact top bar, only below md (the sidebar handles desktop navigation). */}
          <header className="flex h-[52px] shrink-0 items-center gap-3 border-b border-border bg-sidebar px-3 md:hidden">
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => setNavOpen(true)}
              aria-label={t("nav.menu")}
              title={t("nav.menu")}
            >
              <Menu className="h-4 w-4" />
            </Button>
            <div className="flex items-center gap-2.5">
              <img
                src="/logo.png"
                alt="DeepMate"
                className="h-6 w-6 rounded-sm inline dark:hidden"
              />
              <img
                src="/logo-dark.png"
                alt=""
                aria-hidden
                className="hidden dark:inline h-6 w-6 rounded-sm border border-border"
              />
              <span className="text-brand text-text">DeepMate</span>
            </div>
            <div className="flex-1" />
            {harnessName && (
              <div className="flex min-w-0 items-center gap-1.5 text-caption text-text-faint">
                <Cpu className="h-3.5 w-3.5 shrink-0" />
                <span className="truncate">{harnessName}</span>
              </div>
            )}
          </header>

          {/* Page content: the only scroll region. */}
          <main className="min-h-0 flex-1 overflow-y-auto">
            {view === "overview" && <OverviewPage onNavigate={navigate} />}
            {view === "plugins" && <PluginsPage />}
            {view === "settings" && <SettingsPage />}
          </main>
        </div>

        {/* Mobile navigation drawer. */}
        <MobileNav
          open={navOpen}
          view={view}
          onNavigate={navigate}
          onClose={() => setNavOpen(false)}
        />
      </div>
    </TooltipProvider>
  );
}
