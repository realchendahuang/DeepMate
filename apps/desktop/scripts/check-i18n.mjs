// Localization gate: both catalogs must describe the same UI.
//
// A missing key does not crash anything — i18next just renders the key path —
// which is exactly why it went unnoticed until someone opened the screen. The
// rules enforced here:
//   - every key present in one catalog exists in the other
//   - no empty values
//   - interpolation variables match between translations
//   - the English catalog is actually English (no untranslated zh values)
//   - every `t("...")` the source references resolves to real text in both
//     languages, rather than falling through to the key path
//
// Plural forms are the documented exception: English carries `_one`/`_other`
// while Chinese uses a single `_other`, and i18next resolves `key` to
// `key_other` at runtime. So a base key in one catalog may be represented by
// its plural variants in the other; the comparison is done on the base name.
//
// Exits non-zero with a readable list when a rule is broken.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import i18next from "i18next";

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

// The English catalog must not carry Chinese text. This is how a half-done
// translation looks: the key exists, so the parity check above is happy, and
// an English user reads Chinese. (It happened: `settings.modality.*` landed
// with the zh wording in both catalogs.)
const CJK = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;
for (const [key, entries] of en) {
  for (const { path, value } of entries) {
    if (typeof value === "string" && CJK.test(value)) {
      problems.push(`en value contains Chinese text: ${path} = ${JSON.stringify(value)}`);
    }
  }
}

// Keys the source actually asks for. A key that exists in both catalogs but
// is spelled differently in the code still renders as the key path, and
// checking parity alone cannot see that.
function sourceFiles(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) sourceFiles(path, out);
    else if (/\.(ts|tsx)$/.test(path) && !path.includes("locales")) out.push(path);
  }
  return out;
}

const referenced = new Set();
for (const file of sourceFiles(join(here, "..", "src"))) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(/\bt\(\s*"([a-zA-Z0-9_.]+)"/g)) {
    referenced.add(match[1]);
  }
}

async function keysRenderingAsPaths(language, resource, label) {
  const instance = i18next.createInstance();
  await instance.init({
    resources: { [language]: { translation: resource } },
    lng: language,
    fallbackLng: false,
    interpolation: { escapeValue: false },
  });
  const unresolved = [];
  for (const key of referenced) {
    // `count` lets i18next pick the plural form, matching how the UI calls it.
    if (instance.t(key, { count: 1 }) === key) unresolved.push(key);
  }
  return unresolved.map((key) => `${label}: ${key} renders as a raw key path`);
}

problems.push(
  ...(await keysRenderingAsPaths("en", read("en"), "en")),
  ...(await keysRenderingAsPaths("zh", read("zh"), "zh")),
);

if (problems.length > 0) {
  console.error("i18n catalogs disagree:\n" + problems.map((line) => `  - ${line}`).join("\n"));
  process.exit(1);
}

console.log(`i18n catalogs agree (${en.size} keys, ${referenced.size} referenced)`);
