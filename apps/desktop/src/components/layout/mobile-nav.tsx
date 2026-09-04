import { useTranslation } from "react-i18next";
import type { View } from "./nav";
import { SidebarNav } from "./app-sidebar";
import { Sheet, SheetContent, SheetTitle } from "../ui/sheet";

interface MobileNavProps {
  open: boolean;
  view: View;
  onNavigate: (view: View) => void;
  onClose: () => void;
}

// The mobile navigation drawer, built on the shadcn Sheet (Radix Dialog):
// focus trap, Escape to close, scrim click and body scroll locking included.
export function MobileNav({ open, view, onNavigate, onClose }: MobileNavProps) {
  const { t } = useTranslation();

  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent side="left" className="p-0" showCloseButton={false}>
        <SheetTitle className="sr-only">{t("nav.menu")}</SheetTitle>
        <div className="flex h-[52px] shrink-0 items-center justify-between gap-2 border-b border-border px-3">
          <div className="flex items-center gap-2.5">
            <img src="/logo.png" alt="DeepMate" className="h-6 w-6" />
            <span className="text-brand text-text">DeepMate</span>
          </div>
        </div>
        <SidebarNav view={view} onNavigate={onNavigate} />
      </SheetContent>
    </Sheet>
  );
}
