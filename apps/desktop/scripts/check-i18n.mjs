// Localization gate: both catalogs must describe the same UI.
//
// A missing key does not crash anything — i18next just renders the key path —
// which is exactly why it went unnoticed until someone opened the screen. The
// rules enforced here:
//   - every key present in one catalog exists in the other
//   - no empty values
//   - interpolation variables match between translations
//
// Plural forms are the documented exception: English carries `_one`/`_other`
// while Chinese uses a single `_other`, and i18next resolves `key` to
// `key_other` at runtime. So a base key in one catalog may be represented by
// its plural variants in the other; the comparison is done on the base name.
//
// Exits non-zero with a readable list when a rule is broken.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const localesDir = join(here, "..", "src", "locales");

const read = (name) => JSON.parse(readFileSync(join(localesDir, `${name}.json`), "utf8"));

// Flatten to dotted paths, recording plural variants under their base name.
function flatten(value, prefix = "", out = new Map()) {
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object" && !Array.isArray(child)) {
      flatten(child, path, out);
    } else {
      const base = path.replace(/_(zero|one|two|few|many|other)$/, "");
      const entry = out.get(base) ?? [];
      entry.push({ path, value: child });
      out.set(base, entry);
    }
  }
  return out;
}

const en = flatten(read("en"));
const zh = flatten(read("zh"));

const problems = [];

for (const [key, entries] of en) {
  if (!zh.has(key)) problems.push(`zh is missing: ${key}`);
  for (const { path, value } of entries) {
    if (typeof value !== "string" || value.trim() === "") {
      problems.push(`empty value: en ${path}`);
    }
  }
}
for (const [key, entries] of zh) {
  if (!en.has(key)) problems.push(`en is missing: ${key}`);
  for (const { path, value } of entries) {
    if (typeof value !== "string" || value.trim() === "") {
      problems.push(`empty value: zh ${path}`);
    }
  }
}

// Interpolation variables must agree between the two languages.
const variables = (text) =>
  [...text.matchAll(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g)].map((m) => m[1]).sort();

for (const [key, enEntries] of en) {
  const zhEntries = zh.get(key);
  if (!zhEntries) continue;
  const enVars = new Set(enEntries.flatMap(({ value }) => variables(value)));
  const zhVars = new Set(zhEntries.flatMap(({ value }) => variables(value)));
  for (const variable of enVars) {
    if (!zhVars.has(variable)) {
      problems.push(`placeholder {{${variable}}} is missing in zh: ${key}`);
    }
  }
  for (const variable of zhVars) {
    if (!enVars.has(variable)) {
      problems.push(`placeholder {{${variable}}} is missing in en: ${key}`);
    }
  }
}

if (problems.length > 0) {
  console.error("i18n catalogs disagree:\n" + problems.map((line) => `  - ${line}`).join("\n"));
  process.exit(1);
}

console.log(`i18n catalogs agree (${en.size} keys)`);
