// The scenario rail — the outermost column: one avatar per scenario, a
// trailing "+" to create one (which owns its own dialog), and system settings
// pinned to the bottom. Right-clicking an avatar manages that scenario
// (rename / description / delete) through the same dialogs the overview hero
// uses. No product logo and no home button — the app always lives inside a
// scenario, so the rail is pure switcher.

import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Settings } from "lucide-react";
import { useRouterStore, SCENARIO_HOME } from "../router";
import { useScenarioStore } from "../store/scenarios";
import { useRuntimeStore } from "../store/runtime";
import { cn } from "@/shared/lib/utils";
import { isDefaultScenario, scenarioInitial } from "@/shared/lib/scenario";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuTrigger,
} from "@/shared/ui/context-menu";
import { NewScenarioDialog } from "@/features/scenarios/new-scenario-dialog";
import {
  ScenarioManageDialogs,
  ScenarioManageMenu,
  useScenarioManage,
} from "@/features/scenarios/scenario-manage";

function RailButton({
  active,
  onClick,
  label,
  running,
  menu,
  children,
  className,
}: {
  active?: boolean;
  onClick: () => void;
  label: string;
  running?: boolean;
  menu?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <Tooltip>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <TooltipTrigger asChild>
            <div className="group/rail relative flex w-rail shrink-0 justify-center">
              <span
                className={cn(
                  "absolute top-1/2 left-0 w-1 -translate-y-1/2 rounded-r-full bg-text transition-all duration-150",
                  active
                    ? "h-9 opacity-100"
                    : "h-0 opacity-0 group-hover/rail:h-5 group-hover/rail:opacity-70",
                )}
                aria-hidden
              />
              <button
                type="button"
                onClick={onClick}
                aria-label={label}
                aria-current={active ? "true" : undefined}
                className={cn(
                  "relative flex h-12 w-12 shrink-0 items-center justify-center text-small font-bold transition-all duration-150 active:scale-95",
                  active
                    ? "rounded-2xl bg-accent text-on-accent shadow-sm"
                    : "rounded-full bg-panel-2 text-text-dim hover:rounded-2xl hover:bg-hover hover:text-text",
                  className,
                )}
              >
                {children}
              </button>
              {running !== undefined && (
                <span className="pointer-events-none absolute right-2.5 bottom-0.5 flex h-3.5 w-3.5 items-center justify-center">
                  {running && (
                    <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-pass opacity-75" />
                  )}
                  <span
                    className={cn(
                      "relative h-3 w-3 rounded-full border-2 border-rail",
                      running ? "bg-pass" : "bg-neutral",
                    )}
                    aria-hidden
                  />
                </span>
              )}
            </div>
          </TooltipTrigger>
        </ContextMenuTrigger>
        {menu && <ContextMenuContent>{menu}</ContextMenuContent>}
      </ContextMenu>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

export function ScenarioRail() {
  const { t } = useTranslation();
  const route = useRouterStore((s) => s.route);
  const navigate = useRouterStore((s) => s.navigate);
  const openScenario = useRouterStore((s) => s.openScenario);
  const openSettings = useRouterStore((s) => s.openSettings);
  const profiles = useScenarioStore((s) => s.profiles);
  const selectedScenario = useScenarioStore((s) => s.selectedScenario);
  const instances = useRuntimeStore((s) => s.instances);
  const manage = useScenarioManage();
  const [creating, setCreating] = useState(false);

  const settingsActive = route.kind === "settings";
  const sceneActive = route.kind === "scenario";

  const scenes = (
    <>
      {profiles.map((profile) => {
        const instance = instances.find((item) => item.profile === profile.id);
        const running = instance?.status === "running";
        const active = sceneActive && selectedScenario === profile.id;
        const label = isDefaultScenario(profile)
          ? t("scene.defaultName", { name: profile.name })
          : profile.name;
        return (
          <RailButton
            key={profile.id}
            active={active}
            running={running}
            label={label}
            onClick={() => openScenario(profile.id)}
            menu={
              <ScenarioManageMenu
                profile={profile}
                busy={manage.busy}
                open={manage.open}
              />
            }
          >
            {scenarioInitial(profile.name)}
          </RailButton>
        );
      })}
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="relative flex w-rail shrink-0 justify-center">
            <button
              type="button"
              onClick={() => setCreating(true)}
              aria-label={t("settings.addProfile")}
              title={t("settings.addProfile")}
              className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-panel-2 text-text-faint transition-[border-radius,background-color,color] hover:rounded-2xl hover:bg-pass/15 hover:text-pass"
            >
              <Plus className="h-5 w-5" />
            </button>
          </div>
        </TooltipTrigger>
        <TooltipContent side="right">{t("settings.addProfile")}</TooltipContent>
      </Tooltip>
    </>
  );

  return (
    <>
      <aside className="hidden h-full w-rail shrink-0 flex-col items-center border-r border-border bg-rail md:flex">
        <div className="no-scrollbar flex min-h-0 w-full flex-1 flex-col items-center gap-2 overflow-y-auto py-3">
          {scenes}
        </div>
        <div className="h-[1px] w-8 bg-border" />
        <div className="flex w-full justify-center py-3">
          <RailButton
            active={settingsActive}
            label={t("settings.title")}
            onClick={() => openSettings("preferences")}
          >
            <Settings className="h-5 w-5" />
          </RailButton>
        </div>
      </aside>
      <div className="no-scrollbar flex w-full shrink-0 items-center gap-2 overflow-x-auto border-b border-border bg-rail px-2 py-2 md:hidden">
        {scenes}
        <RailButton
          active={settingsActive}
          label={t("settings.title")}
          onClick={() => openSettings("preferences")}
        >
          <Settings className="h-5 w-5" />
        </RailButton>
      </div>

      <NewScenarioDialog
        open={creating}
        onOpenChange={setCreating}
        onCreated={() => navigate(SCENARIO_HOME)}
      />

      <ScenarioManageDialogs manage={manage} />
    </>
  );
}
