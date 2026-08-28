# Curated Plugin List

This directory holds the curated plugin list that powers the **Curated** source
in the DeepMate plugin marketplace. The list is maintained in this repository;
anyone can propose an addition by opening a pull request.

## How it works

- `curated.json` is fetched by DeepMate at runtime from the raw GitHub URL of
  this file and cached locally (same cache mechanism as marketplace search
  results). The list is the *curation*; installation still happens through the
  npm registry, so version ranges, compatibility checks and updates keep
  working unchanged.
- Entries are shown in the marketplace as the **Curated** source, separate from
  the raw npm search results (**Community**).
- The list is versioned with a `schema` field. DeepMate refuses to load a
  list with a newer schema it does not understand, so the format can evolve
  without breaking older clients.

## Proposing a plugin

Open a pull request that adds an entry to `curated.json`. Each entry needs:

| Field        | Required | Meaning                                                        |
| ------------ | -------- | -------------------------------------------------------------- |
| `name`       | yes      | npm package name, e.g. `dsh-mnemon` or `@scope/name`            |
| `version`    | yes      | recommended semver range, e.g. `^0.2`                          |
| `description`| yes      | one sentence, what the plugin does                              |
| `repository` | no       | source repository URL                                          |
| `publisher`  | no       | npm publisher name                                             |
| `added`      | yes      | date the entry was added, `YYYY-MM-DD`                         |
| `trust`      | no       | `official` for DeepSeek Harness vendor plugins, `vetted` for community plugins reviewed by the maintainers |
| `category`   | no       | functional category for the storefront filter: `memory`, `vision`, `mcp`, `chat`, `web`, `web-ui`, `terminal`, `tui`, `remote`, `office`, `dev`, `stats`, `security`, `automation` (official plugins use `official`) |

Guidelines:

- The plugin must be published on the npm registry and installable with
  `deepmate plugin install <name>`.
- Prefer a `version` range that matches the harness version DeepMate targets.
- Keep entries sorted by `name` so diffs stay reviewable.
- Bump `updated` to the date of the change.

## First batch

The initial list (2026-08-28) was assembled by cross-referencing the
community's existing curated indexes — the AdamPlatin123/awesome-dsh-plugins
radar (human-curated Top 50) and the dshworks/awesome-dsh-plugins registry
(runtime-verified entries) — against the npm registry. Every entry was
verified to exist on npm with the version recorded here. Suite/wizard
packages (e.g. `dsh-zcf`, `@rezti/dsh-rez-suite`) were left out on purpose:
the list favors single-purpose plugins that do one thing well.

## Format

```json
{
  "schema": 1,
  "updated": "2026-08-28",
  "plugins": [
    {
      "name": "dsh-mnemon",
      "version": "^0.2",
      "description": "Memory plugin for DeepSeek Harness",
      "repository": "https://github.com/example/dsh-mnemon",
      "publisher": "someone",
      "added": "2026-08-28",
      "trust": "vetted"
    }
  ]
}
```
