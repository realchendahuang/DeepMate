// Label + control grouping used by the provider & model editors.

import type { ReactNode } from "react";
import { Label } from "@/shared/ui/label";

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-1">
      <Label>{label}</Label>
      {children}
    </div>
  );
}
