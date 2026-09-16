// Scenario-domain helpers shared by the shell and the scenario views.

import type { Profile, Surface } from "@/shared/api/api";

// The default scenario the harness ships with. It is the fixed `web` profile:
// the runtime protects it from rename/remove, it keeps the legacy URL and it
// answers "web" when its surface has never been probed.
export const DEFAULT_SCENARIO_ID = "web";

export function isDefaultScenario(profile: Pick<Profile, "id">): boolean {
  return profile.id === DEFAULT_SCENARIO_ID;
}

// The harness falls back to `bundles: a, b, c` when a profile has no
// description. Split that into chips; any other string is a real blurb.
export function scenarioLayers(description: string | null): {
  bundles: string[];
  blurb: string | null;
} {
  if (!description) return { bundles: [], blurb: null };
  const trimmed = description.trim();
  const match = /^bundles:\s*(.*)$/is.exec(trimmed);
  if (!match) return { bundles: [], blurb: trimmed };
  const bundles = match[1]
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
  return { bundles, blurb: null };
}

export function scenarioInitial(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "?";
  return Array.from(trimmed)[0]!.toUpperCase();
}

// Resolve a scenario's surface. The runtime reports "undetermined" when the
// scenario never ran; fall back to the default profile (web) or the
// declared bundles (headless → task, web-app/web-ui → web).
export function inferSurface(profile: Profile, surface: Surface): Surface {
  if (surface !== "undetermined") return surface;
  if (isDefaultScenario(profile)) return "web";
  const { bundles } = scenarioLayers(profile.description);
  if (bundles.some((id) => id.includes("headless"))) return "task";
  if (bundles.some((id) => id.includes("web-app") || id.includes("web-ui"))) return "web";
  return surface;
}
