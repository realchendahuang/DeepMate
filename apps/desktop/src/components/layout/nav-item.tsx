import { useTranslation } from "react-i18next";
import type { LucideIcon } from "lucide-react";
import { cn } from "../../lib/utils";

interface NavItemProps {
  icon: LucideIcon;
  labelKey: string;
  active: boolean;
  onClick: () => void;
}

export function NavItem({ icon: Icon, labelKey, active, onClick }: NavItemProps) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-8 w-full items-center gap-2 rounded-md px-2 text-body font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent",
        active ? "bg-accent-soft text-accent" : "text-text-dim hover:bg-hover hover:text-text",
      )}
    >
      <Icon className="h-4 w-4 shrink-0" />
      <span className="truncate">{t(labelKey)}</span>
    </button>
  );
}
