// The chrome: scenario rail (left), section sidebar, and the routed content
// column. Navigation lives in the router store; this component only owns the
// mobile drawer state.

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import { Menu } from "lucide-react";
import { Button } from "@/shared/ui/button";
import { TooltipProvider } from "@/shared/ui/tooltip";
import { useRouterStore, routeKey } from "../router";
import { RouteView } from "../route-view";
import { AppSidebar } from "./app-sidebar";
import { MobileNav } from "./mobile-nav";
import { ScenarioRail } from "./scenario-rail";
import { pageVariants } from "@/shared/lib/motion";

export function AppShell() {
  const { t } = useTranslation();
  const route = useRouterStore((s) => s.route);
  const direction = useRouterStore((s) => s.direction);
  const [navOpen, setNavOpen] = useState(false);
  const mainRef = useRef<HTMLElement>(null);

  return (
    <TooltipProvider>
      <div className="flex h-screen flex-col bg-bg text-text md:flex-row">
        <ScenarioRail />

        <AppSidebar />

        <div className="flex min-w-0 flex-1 flex-col">
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
            <span className="text-brand text-text">DeepMate</span>
            <div className="flex-1" />
          </header>

          <main ref={mainRef} className="min-h-0 flex-1 overflow-hidden">
            <AnimatePresence
              mode="wait"
              initial={false}
              custom={direction}
              onExitComplete={() => mainRef.current?.scrollTo({ top: 0 })}
            >
              <motion.div
                key={routeKey(route)}
                custom={direction}
                variants={pageVariants}
                initial="enter"
                animate="center"
                exit="exit"
                className="h-full overflow-y-auto"
              >
                <RouteView route={route} />
              </motion.div>
            </AnimatePresence>
          </main>
        </div>

        <MobileNav open={navOpen} onClose={() => setNavOpen(false)} />
      </div>
    </TooltipProvider>
  );
}
