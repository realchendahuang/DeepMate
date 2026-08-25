import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import type { View } from "./nav";
import { SidebarNav } from "./app-sidebar";
import { Button } from "../ui/button";

interface MobileNavProps {
  open: boolean;
  view: View;
  onNavigate: (view: View) => void;
  onClose: () => void;
}

export function MobileNav({ open, view, onNavigate, onClose }: MobileNavProps) {
  const { t } = useTranslation();

  // Close on Escape while the drawer is open.
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 md:hidden">
      {/* Scrim: click to close. */}
      <div
        className="absolute inset-0 bg-black/50 animate-in fade-in duration-150"
        onClick={onClose}
      />
      {/* Drawer: same navigation as the desktop sidebar. */}
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("nav.menu")}
        className="absolute inset-y-0 left-0 flex w-sidebar flex-col border-r border-border bg-sidebar animate-in slide-in-from-left duration-150"
      >
        <div className="flex h-[52px] shrink-0 items-center justify-between gap-2 border-b border-border px-3">
          <div className="flex items-center gap-2.5">
            <img src="/logo.png" alt="DeepMate" className="h-6 w-6" />
            <span className="text-brand text-text">DeepMate</span>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={onClose}
            aria-label={t("nav.close")}
            title={t("nav.close")}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
        <SidebarNav view={view} onNavigate={onNavigate} />
      </div>
    </div>
  );
}
