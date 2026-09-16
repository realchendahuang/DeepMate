// Friendly error mapping. Tauri commands surface errors as JSON envelopes
// `{"code":"<stable code>","message":"..."}` for core errors (see the
// desktop's `command_error` helper); the stable codes map to i18n keys
// directly, while legacy plain strings (update plumbing) keep the pattern
// matching below. Anything unknown falls back to a generic line. The mapped
// text is a stable key that callers translate via i18n (`common.errors.*`).

const CODE_KEYS: Record<string, string> = {
  not_found: "notFound",
  timeout: "timeout",
  network: "network",
  // These classifications have no friendlier wording than the generic line.
  io: "generic",
  spawn_failed: "generic",
  command_failed: "generic",
  invalid_state: "generic",
  unsupported: "generic",
};

const KNOWN_PATTERNS: Array<{ pattern: RegExp; key: string }> = [
  { pattern: /incompatible/i, key: "incompatible" },
  { pattern: /not found/i, key: "notFound" },
  { pattern: /already (exists|installed)/i, key: "alreadyExists" },
  { pattern: /network|offline|timed? ?out|ECONNREFUSED|ENOTFOUND/i, key: "network" },
  { pattern: /permission|denied|forbidden/i, key: "permission" },
  { pattern: /checksum|signature|verify/i, key: "checksum" },
];

// The JSON envelope core errors are wrapped in; `null` for legacy strings.
function parseEnvelope(raw: unknown): { code?: string; message: string } | null {
  const text = typeof raw === "string" ? raw : raw instanceof Error ? raw.message : null;
  if (!text || !text.startsWith("{")) return null;
  try {
    const value = JSON.parse(text) as { code?: unknown; message?: unknown };
    if (value && typeof value.message === "string") {
      return {
        code: typeof value.code === "string" ? value.code : undefined,
        message: value.message,
      };
    }
  } catch {
    // Not an envelope: keep the string as-is.
  }
  return null;
}

export function mapError(raw: unknown): string {
  const parsed = parseEnvelope(raw);
  if (parsed?.code) {
    const key = CODE_KEYS[parsed.code];
    if (key) return key;
  }
  const message = parsed?.message ?? (raw instanceof Error ? raw.message : String(raw));
  for (const { pattern, key } of KNOWN_PATTERNS) {
    if (pattern.test(message)) return key;
  }
  return "generic";
}

// The human-readable message behind a raw error: unwraps the JSON envelope,
// keeps plain strings as they are. Callers that display the raw text
// (details sections) must use this instead of `String(error)`.
export function errorMessage(raw: unknown): string {
  const parsed = parseEnvelope(raw);
  if (parsed) return parsed.message;
  return raw instanceof Error ? raw.message : String(raw);
}
