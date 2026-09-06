import { useTranslation } from "react-i18next";
import { Globe, TerminalSquare } from "lucide-react";
import type { View } from "./nav";
import { SCENARIO_SECTIONS } from "./nav";
import { NavItem } from "./nav-item";
import { useStore } from "../../store";
import { cn } from "../../lib/utils";

interface AppSidebarProps {
  view: View;
  onNavigate: (view: View) => void;
  onOpenScenario: (profile: string) => void;
}

interface SidebarNavProps {
  view: View;
  onNavigate: (view: View) => void;
  onOpenScenario: (profile: string) => void;
  /** Shared layoutId so the active highlight slides between nav items. */
  pillId: string;
}

// The main navigation list shared by the desktop sidebar and the mobile
// drawer. It has two modes:
//
//  * scenario mode (the default): the selected scenario's sections (run,
//    providers & models, plugins), headed by the scenario's name.
//  * all-scenarios mode (the rail's grid button): every scenario as a
//    navigation list, headed "所有场景", with the management page as the
//    content. Clicking a scenario there opens it and returns to scenario
//    mode.
//
// System settings live behind the rail's bottom gear and never occupy this
// list.
export function SidebarNav({ view, onNavigate, onOpenScenario, pillId }: SidebarNavProps) {
  const { t } = useTranslation();
  const selectedScenario = useStore((s) => s.selectedScenario);
  const scenarios = useStore((s) => s.profiles);
  const instances = useStore((s) => s.instances);

  if (view === "scenarios") {
    return (
      <nav aria-label={t("nav.menu")} className="flex-1 space-y-4 overflow-y-auto px-3 py-4">
        <div className="space-y-1">
          <div className="px-2 pb-1 text-caption font-semibold text-text-faint">
            {t("settings.scenarios")}
          </div>
          {scenarios.map((profile) => {
            const instance = instances.find((item) => item.profile === profile.id);
            const running = instance?.status === "running";
            const active = selectedScenario === profile.id;
            return (
              <button
                key={profile.id}
                type="button"
                onClick={() => onOpenScenario(profile.id)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-body transition-colors",
                  active ? "bg-hover font-semibold text-text" : "text-text-dim hover:bg-hover/60",
                )}
              >
                <span
                  className={cn(
                    "h-2 w-2 shrink-0 rounded-full",
                    running ? "bg-pass" : "bg-neutral",
                  )}
                />
                <span className="min-w-0 flex-1 truncate text-left">{profile.name}</span>
                {instance?.surface === "task" ? (
                  <TerminalSquare className="h-3.5 w-3.5 shrink-0 text-text-faint" />
                ) : instance?.surface === "web" ? (
                  <Globe className="h-3.5 w-3.5 shrink-0 text-text-faint" />
                ) : null}
              </button>
            );
          })}
          {scenarios.length === 0 && (
            <div className="px-2 text-caption text-text-faint">{t("overview.noScenarios")}</div>
          )}
        </div>
      </nav>
    );
  }

  const scenarioName =
    scenarios.find((profile) => profile.id === selectedScenario)?.name ?? selectedScenario;

  return (
    <nav aria-label={t("nav.menu")} className="flex-1 space-y-4 overflow-y-auto px-3 py-4">
      <div className="space-y-1">
        <div className="flex items-center justify-between px-2 pb-1">
          <span className="min-w-0 truncate text-caption font-semibold text-text-faint">
            {scenarioName || t("settings.title")}
          </span>
        </div>
        {SCENARIO_SECTIONS.map((item) => (
          <NavItem
            key={item.view}
            icon={item.icon}
            labelKey={item.labelKey}
            active={view === item.view}
            onClick={() => onNavigate(item.view)}
            pillId={pillId}
          />
        ))}
      </div>
    </nav>
  );
}

export function AppSidebar({ view, onNavigate, onOpenScenario }: AppSidebarProps) {
  return (
    <aside className="hidden w-sidebar shrink-0 flex-col border-r border-border bg-sidebar md:flex">
      <div className="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-border px-4">
        <img src="/logo.png" alt="DeepMate" className="h-6 w-6 rounded-sm inline dark:hidden" />
        <img
          src="/logo-dark.png"
          alt=""
          aria-hidden
          className="hidden dark:inline h-6 w-6 rounded-sm border border-border"
        />
        <span className="text-brand text-text">DeepMate</span>
      </div>
      <SidebarNav
        view={view}
        onNavigate={onNavigate}
        onOpenScenario={onOpenScenario}
        pillId="sidebar-nav"
      />
    </aside>
  );
}