import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Cpu } from "lucide-react";
import type { View } from "./nav";
import { NAV_MAIN, NAV_FOOTER } from "./nav";
import { NavItem } from "./nav-item";

interface AppSidebarProps {
  view: View;
  onNavigate: (view: View) => void;
  adapterId?: string;
}

interface SidebarNavProps {
  view: View;
  onNavigate: (view: View) => void;
  footer?: ReactNode;
}

// The navigation list shared by the desktop sidebar and the mobile drawer:
// primary items above, the settings entry (and any footer extras) pinned below.
export function SidebarNav({ view, onNavigate, footer }: SidebarNavProps) {
  const { t } = useTranslation();

  return (
    <>
      <nav className="flex-1 space-y-1 overflow-y-auto px-3 py-4" aria-label={t("nav.menu")}>
        {NAV_MAIN.map((item) => (
          <NavItem
            key={item.view}
            icon={item.icon}
            labelKey={item.labelKey}
            active={view === item.view}
            onClick={() => onNavigate(item.view)}
          />
        ))}
      </nav>
      <div className="border-t border-border p-3">
        <div className="space-y-1">
          {NAV_FOOTER.map((item) => (
            <NavItem
              key={item.view}
              icon={item.icon}
              labelKey={item.labelKey}
              active={view === item.view}
              onClick={() => onNavigate(item.view)}
            />
          ))}
        </div>
        {footer}
      </div>
    </>
  );
}

export function AppSidebar({ view, onNavigate, adapterId }: AppSidebarProps) {
  return (
    <aside className="hidden w-sidebar shrink-0 flex-col border-r border-border bg-sidebar md:flex">
      <div className="flex h-[52px] shrink-0 items-center gap-2.5 border-b border-border px-4">
        <img src="/logo.png" alt="DeepMate" className="h-6 w-6" />
        <span className="text-brand text-text">DeepMate</span>
      </div>
      <SidebarNav
        view={view}
        onNavigate={onNavigate}
        footer={
          adapterId ? (
            <div className="mt-3 flex items-center gap-1.5 px-2 text-caption text-text-faint">
              <Cpu className="h-3.5 w-3.5 shrink-0" />
              <span className="truncate">{adapterId}</span>
            </div>
          ) : undefined
        }
      />
    </aside>
  );
}
