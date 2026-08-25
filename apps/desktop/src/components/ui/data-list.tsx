import * as React from "react";
import { cn } from "../../lib/utils";
import { Card, CardContent } from "./card";

export interface DataListColumn {
  label: string;
  className?: string;
  compact?: boolean;
}

interface DataListProps extends React.HTMLAttributes<HTMLDivElement> {
  columns: DataListColumn[];
  children: React.ReactNode;
}

export function DataList({ columns, children, className, ...props }: DataListProps) {
  return (
    <Card className={cn("overflow-hidden", className)} {...props}>
      <div className="flex items-center gap-3 bg-panel-2 px-4 py-2 text-caption font-bold text-text-faint md:px-5">
        {columns.map((column, index) => (
          <span
            key={column.label}
            className={cn(
              index === 0 ? "min-w-0 flex-1" : "shrink-0",
              column.compact && "hidden lg:block",
              column.className,
            )}
          >
            {column.label}
          </span>
        ))}
      </div>
      <div className="divide-y divide-border">{children}</div>
    </Card>
  );
}

interface DataListRowProps extends React.HTMLAttributes<HTMLDivElement> {
  primary: React.ReactNode;
  secondary?: React.ReactNode;
  detail?: React.ReactNode;
}

export function DataListRow({
  primary,
  secondary,
  detail,
  className,
  ...props
}: DataListRowProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover md:px-5",
        className,
      )}
      {...props}
    >
      <div className="min-w-0 flex-1">
        <div className="truncate text-body font-semibold text-text">{primary}</div>
        {detail && <div className="mt-0.5 truncate text-small text-text-faint lg:hidden">{detail}</div>}
      </div>
      {secondary && <div className="shrink-0 text-small text-text-dim">{secondary}</div>}
      {detail && <div className="hidden min-w-0 flex-1 truncate text-small text-text-faint lg:block">{detail}</div>}
    </div>
  );
}

interface EmptyStateProps {
  icon: React.ReactNode;
  children: React.ReactNode;
}

export function EmptyState({ icon, children }: EmptyStateProps) {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-2 p-4 py-10 text-center">
        {icon}
        <p className="text-body font-semibold text-text-dim">{children}</p>
      </CardContent>
    </Card>
  );
}
