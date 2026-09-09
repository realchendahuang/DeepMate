import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MOTION_DURATION, MOTION_EASE } from "@/shared/lib/motion";

interface NavItemProps {
  icon: LucideIcon;
  labelKey: string;
  active: boolean;
  onClick: () => void;
  /** Shared across siblings so the active highlight slides between items. */
  pillId: string;
  badge?: React.ReactNode;
}

export function NavItem({ icon: Icon, labelKey, active, onClick, pillId, badge }: NavItemProps) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "relative flex h-8 w-full items-center gap-2 rounded-md px-2 text-body font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent",
        active ? "text-accent" : "text-text-dim hover:bg-hover hover:text-text",
      )}
    >
      {active && (
        <motion.span
          layoutId={pillId}
          className="absolute inset-0 rounded-md bg-accent-soft"
          transition={{ duration: MOTION_DURATION.fast, ease: MOTION_EASE.out }}
        />
      )}
      <Icon className="relative h-4 w-4 shrink-0" />
      <span className="relative truncate">{t(labelKey)}</span>
      {badge && <span className="relative ml-auto shrink-0">{badge}</span>}
    </button>
  );
}
