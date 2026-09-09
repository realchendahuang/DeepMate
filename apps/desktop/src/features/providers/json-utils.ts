// Helper functions for validating and preparing JSON strings in provider and model fields.

export function jsonOrNull(text: string): string | null {
  const trimmed = text.trim();
  return trimmed ? trimmed : null;
}

export function jsonValid(draft: string): boolean {
  const trimmed = draft.trim();
  if (!trimmed) return true;
  try {
    JSON.parse(trimmed);
    return true;
  } catch {
    return false;
  }
}
