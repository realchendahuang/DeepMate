// The scenario rail — the outermost column, a workspace switcher like a
// multi-tenant app: one button per scenario (status dot + surface icon),
// the active scenario highlighted, a trailing "+" to create one. Scenarios
// live here, never inside the main navigation. On small screens the rail
// becomes a horizontal strip across the top of the content header.

import { useTranslation } from "react-i18next";
import { Globe, LayoutGrid, Plus, Settings, TerminalSquare } from "lucide-react";
import { useStore } from "../../store";
import { cn } from "../../lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "../ui/tooltip";

export function ScenarioRail({
  onOpen,
  onOpenAll,
  onOpenSettings,
  allActive = false,
}: {
  onOpen: (profile: string) => void;
  onOpenAll: () => void;
  onOpenSettings: () => void;
  allActive?: boolean;
}) {
  const { t } = useTranslation();
  const profiles = useStore((s) => s.profiles);
  const instances = useStore((s) => s.instances);
  const selectedScenario = useStore((s) => s.selectedScenario);

  const rows = (
    <>
      {profiles.map((profile) => {
        const instance = instances.find((item) => item.profile === profile.id);
        const running = instance?.status === "running";
        const active = selectedScenario === profile.id;
        const surface = instance?.surface ?? "undetermined";
        const label = profile.id === "web" ? `${profile.name}（默认）` : profile.name;
        const button = (
          <button
            type="button"
            onClick={() => onOpen(profile.id)}
            aria-label={label}
            aria-current={active ? "true" : undefined}
            className={cn(
              "group relative flex h-9 w-9 shrink-0 items-center justify-center rounded-xl text-sm font-bold transition-colors",
              active
                ? "bg-accent text-on-accent"
                : "bg-panel-2 text-text-dim hover:bg-hover hover:text-text",
            )}
          >
            {/* Running state dot, top-right corner. */}
            <span
              className={cn(
                "absolute -top-0.5 -right-0.5 h-2 w-2 rounded-full border border-bg",
                running ? "bg-pass" : "bg-neutral",
              )}
              aria-hidden
            />
            {surface === "task" ? (
              <TerminalSquare className="h-4 w-4" />
            ) : surface === "web" ? (
              <Globe className="h-4 w-4" />
            ) : (
              profile.name.charAt(0).toUpperCase()
            )}
            <span
              className={cn(
                "absolute top-1/2 -left-1 h-6 w-1 -translate-y-1/2 rounded-full bg-accent transition-opacity",
                active ? "opacity-100" : "opacity-0",
              )}
              aria-hidden
            />
          </button>
        );
        return (
          <Tooltip key={profile.id}>
            <TooltipTrigger asChild>{button}</TooltipTrigger>
            <TooltipContent side="right">{label}</TooltipContent>
          </Tooltip>
        );
      })}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={() => onOpen("")}
            aria-label={t("settings.addProfile")}
            title={t("settings.addProfile")}
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-dashed border-border-strong text-text-faint transition-colors hover:border-accent hover:text-accent"
          >
            <Plus className="h-4 w-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">{t("settings.addProfile")}</TooltipContent>
      </Tooltip>
    </>
  );

  return (
    <>
      {/* Desktop: the outermost vertical column. Scenarios on top, the
          system-wide settings pinned to the bottom-left corner. */}
      <aside className="hidden shrink-0 flex-col items-center gap-2 overflow-y-auto border-r border-border bg-sidebar py-3 md:flex">
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={onOpenAll}
              aria-label={t("settings.scenarios")}
              title={t("settings.scenarios")}
              className={cn(
                "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl transition-colors",
                allActive
                  ? "bg-accent text-on-accent"
                  : "text-text-faint hover:bg-hover hover:text-text",
              )}
            >
              <LayoutGrid className="h-4 w-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="right">{t("settings.scenarios")}</TooltipContent>
        </Tooltip>
        <span className="h-px w-6 bg-border" aria-hidden />
        {rows}
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={onOpenSettings}
              aria-label={t("settings.title")}
              title={t("settings.title")}
              className="mt-auto flex h-9 w-9 shrink-0 items-center justify-center rounded-xl text-text-faint transition-colors hover:bg-hover hover:text-text"
            >
              <Settings className="h-4 w-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="right">{t("settings.title")}</TooltipContent>
        </Tooltip>
      </aside>
      {/* Mobile: a horizontal strip above the content header. */}
      <div className="flex shrink-0 items-center gap-2 overflow-x-auto border-b border-border bg-sidebar px-3 py-2 md:hidden">
        {rows}
      </div>
    </>
  );
}