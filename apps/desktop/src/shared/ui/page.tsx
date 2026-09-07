import * as React from "react";
import { ArrowLeft } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Button } from "./button";

// The single scrollable page column. Fluid up to the `content` width so the
// column tracks window resizes; past the cap it centers (Notion-style) so the
// extra space splits evenly instead of pooling on one side.
export function PageBody({
  className,
  full = false,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & { full?: boolean }) {
  return (
    <div
      className={cn(
        "flex w-full flex-col",
        full
          ? "h-full min-h-0 overflow-hidden p-4 md:p-5"
          : "mx-auto max-w-content px-4 py-5 md:px-6 md:py-6",
        className,
      )}
      {...props}
    />
  );
}

interface PageHeaderProps extends React.HTMLAttributes<HTMLDivElement> {
  title: string;
  onBack?: () => void;
  backLabel?: string;
  actions?: React.ReactNode;
}

export function PageHeader({
  title,
  onBack,
  backLabel,
  actions,
  className,
  ...props
}: PageHeaderProps) {
  return (
    <header
      className={cn("flex flex-wrap items-center justify-between gap-3", className)}
      {...props}
    >
      <div className="flex min-w-0 items-center gap-2">
        {onBack && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={onBack}
            aria-label={backLabel}
            title={backLabel}
          >
            <ArrowLeft className="h-4 w-4" />
          </Button>
        )}
        <h1 className="truncate text-title font-bold text-text">{title}</h1>
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}

interface SectionHeaderProps extends React.HTMLAttributes<HTMLDivElement> {
  title: string;
  actions?: React.ReactNode;
}

export function SectionHeader({ title, actions, className, ...props }: SectionHeaderProps) {
  return (
    <div className={cn("flex flex-wrap items-center justify-between gap-2", className)} {...props}>
      <h2 className="text-heading font-semibold text-text">{title}</h2>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}
