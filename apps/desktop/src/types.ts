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
}

export interface Model {
  id: string;
  name: string;
  provider: string | null;
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
