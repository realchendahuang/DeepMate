import * as React from "react";
import { Card, CardContent } from "./card";

// A muted placeholder for empty lists. `action` renders an optional call to
// action (e.g. a "browse the market" button) under the message.
export function EmptyState({
  icon,
  children,
  action,
  className,
}: {
  icon: React.ReactNode;
  children: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <Card className={className}>
      <CardContent className="flex flex-col items-center gap-2 p-4 py-10 text-center">
        {icon}
        <p className="text-body font-semibold text-text-dim">{children}</p>
        {action && <div className="mt-1">{action}</div>}
      </CardContent>
    </Card>
  );
}
