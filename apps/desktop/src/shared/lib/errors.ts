// Friendly error mapping. Tauri commands surface errors as raw strings from
// the Rust side; known failure modes get a human-readable message, anything
// else falls back to a generic line. The mapped text is a stable key that
// callers translate via i18n (`common.errors.*`).

const KNOWN_PATTERNS: Array<{ pattern: RegExp; key: string }> = [
  { pattern: /incompatible/i, key: "incompatible" },
  { pattern: /not found/i, key: "notFound" },
  { pattern: /already (exists|installed)/i, key: "alreadyExists" },
  { pattern: /network|offline|timed? ?out|ECONNREFUSED|ENOTFOUND/i, key: "network" },
  { pattern: /permission|denied|forbidden/i, key: "permission" },
  { pattern: /checksum|signature|verify/i, key: "checksum" },
];

export function mapError(raw: unknown): string {
  const message = raw instanceof Error ? raw.message : String(raw);
  for (const { pattern, key } of KNOWN_PATTERNS) {
    if (pattern.test(message)) return key;
  }
  return "generic";
}
