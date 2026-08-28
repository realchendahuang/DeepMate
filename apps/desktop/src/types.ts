// TypeScript mirrors of the Rust core models (crates/deepmate-core/src/model.rs).
// Field names and enum variants match the serde snake_case serialization.

export type RuntimeStatusKind =
  | "unknown"
  | "installed"
  | "running"
  | "stopped"
  | "error";

export interface RuntimeStatus {
  kind: RuntimeStatusKind;
  pid: number | null;
  message: string | null;
}

export interface HarnessInfo {
  id: string;
  name: string;
  version: string | null;
  adapter_version: string;
}

export interface Detection {
  found: boolean;
  harness: HarnessInfo | null;
  detail: string | null;
}

export type CheckStatus = "pass" | "warn" | "fail" | "skip";

export interface DoctorCheck {
  id: string;
  status: CheckStatus;
  summary: string;
  details: string | null;
  suggested_action: string | null;
}

export interface DoctorReport {
  adapter_id: string;
  checks: DoctorCheck[];
}

export interface Profile {
  id: string;
  name: string;
  description: string | null;
}

export interface Provider {
  id: string;
  name: string;
  kind: string;
  api: string | null;
  base_url: string | null;
  api_key_env: string | null;
  compat: string | null;
}

export interface Model {
  id: string;
  name: string;
  provider: string | null;
  context_window: number | null;
  max_tokens: number | null;
  input: string[] | null;
  reasoning_efforts: string | null;
  compat: string | null;
}

export interface Plugin {
  id: string;
  name: string;
  version: string | null;
  enabled: boolean;
  profile: string;
  latest: string | null;
  outdated: boolean;
}

export type MarketSource = "curated" | "community";

export interface MarketEntry {
  id: string;
  name: string;
  description: string | null;
  version: string | null;
  source: MarketSource;
  repository: string | null;
  publisher: string | null;
  updated: string | null;
  // Functional category (e.g. "memory", "vision", "mcp"), when the source
  // publishes one. Absent for raw npm search results.
  category: string | null;
  // Trust scores in 0..1, when the registry publishes them.
  popularity: number | null;
  quality: number | null;
}

export type CompatStatus = "compatible" | "incompatible" | "unknown";

// Result of checking a market package against the detected harness.
export interface CompatReport {
  status: CompatStatus;
  harness_version: string | null;
  required_range: string | null;
  message: string;
}

export interface MarketSourceInfo {
  id: string;
  name: string;
  description: string;
  source: MarketSource;
}

export interface CapabilityCounts {
  profiles: number | null;
  providers: number | null;
  models: number | null;
  plugins: number | null;
}

export interface Overview {
  detection: Detection;
  status: RuntimeStatus;
  counts: CapabilityCounts;
}

export interface AdapterCapabilities {
  runtime: boolean;
  profiles: boolean;
  providers: boolean;
  models: boolean;
  plugins: boolean;
  marketplace: boolean;
  skills: boolean;
  mcp: boolean;
  snapshots: boolean;
}

// The persisted preferences restored on startup.
export interface UiPrefs {
  language: string;
  theme: string;
  check_updates: boolean;
  notify_updates: boolean;
  close_to_tray: boolean;
}

// A newer DeepMate release found by the update check.
export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  url: string;
  published_at: string;
}
