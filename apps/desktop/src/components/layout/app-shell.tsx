import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import { Menu } from "lucide-react";
import { Button } from "../ui/button";
import { TooltipProvider } from "../ui/tooltip";
import type { View } from "./nav";
import { useStore } from "../../store";
import { AppSidebar } from "./app-sidebar";
import { MobileNav } from "./mobile-nav";
import { ScenarioRail } from "./scenario-rail";
import { SettingsPage } from "../../pages/Settings";
import { ScenariosAdmin } from "../../pages/Scenarios";
import { ScenarioHome } from "../scenario/scenario-home";
import { pageVariants } from "../../lib/motion";

// Nav order drives the slide direction of page transitions.
const VIEW_ORDER: View[] = [
  "scenarios",
  "run",
  "providers",
  "plugins",
  "diagnostics",
  "snapshots",
  "preferences",
  "about",
];
const SETTING_VIEWS: View[] = ["diagnostics", "snapshots", "preferences", "about"];

export function AppShell() {
  const { t } = useTranslation();
  const setSelectedScenario = useStore((s) => s.setSelectedScenario);
  // The scenario's run page is the app's landing view.
  const [view, setView] = useState<View>("run");
  // +1 when the new view sits later in the nav order (it slides in from the
  // right), -1 when it sits earlier.
  const [direction, setDirection] = useState<1 | -1>(1);
  const [navOpen, setNavOpen] = useState(false);
  const mainRef = useRef<HTMLElement>(null);

  const navigate = (next: View) => {
    if (next === view) return;
    setDirection(VIEW_ORDER.indexOf(next) >= VIEW_ORDER.indexOf(view) ? 1 : -1);
    setView(next);
    setNavOpen(false);
  };

  // The scenario rail switches tenants. The current section is kept when it
  // belongs to the scenario; a system settings page hands over to the run
  // page of the newly selected scenario.
  const openScenario = (profile: string) => {
    setSelectedScenario(profile);
    if (SETTING_VIEWS.includes(view)) navigate("run");
  };

  return (
    <TooltipProvider>
      <div className="flex h-screen bg-bg text-text">
        {/* The scenario rail: the outermost column, a workspace switcher
            with the system settings pinned to its bottom-left corner. */}
        <ScenarioRail
          onOpen={openScenario}
          onOpenAll={() => navigate("scenarios")}
          onOpenSettings={() => navigate("preferences")}
          allActive={view === "scenarios"}
        />

        {/* Desktop navigation. */}
        <AppSidebar view={view} onNavigate={navigate} onOpenScenario={openScenario} />

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
          </header>

          {/* Page content: the only scroll region. */}
          <main ref={mainRef} className="min-h-0 flex-1 overflow-y-auto">
            {/* The old view exits, then the next enters (directional slide +
                fade). Scroll resets in the gap so every page starts at the
                top. */}
            <AnimatePresence
              mode="wait"
              initial={false}
              custom={direction}
              onExitComplete={() => mainRef.current?.scrollTo({ top: 0 })}
            >
              <motion.div
                key={view}
                custom={direction}
                variants={pageVariants}
                initial="enter"
                animate="center"
                exit="exit"
                className="min-h-full"
              >
                {view === "scenarios" ? (
                  <ScenariosAdmin onOpenScenario={openScenario} />
                ) : view === "run" || view === "providers" || view === "plugins" ? (
                  <ScenarioHome section={view} />
                ) : (
                  <SettingsPage section={view} />
                )}
              </motion.div>
            </AnimatePresence>
          </main>
        </div>

        {/* Mobile navigation drawer. */}
        <MobileNav
          open={navOpen}
          view={view}
          onNavigate={navigate}
          onOpenScenario={openScenario}
          onClose={() => setNavOpen(false)}
        />
      </div>
    </TooltipProvider>
  );
}
