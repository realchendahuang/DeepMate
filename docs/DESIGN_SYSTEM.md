# DeepMate Desktop Design System

The design system for the DeepMate desktop shell (Tauri 2 + React +
Tailwind): tokens, components, page patterns, and the rules that keep the UI
consistent. Every visual value must come from the design tokens in
`src/styles.css` + `tailwind.config.ts`; pages and components never invent
their own colors, sizes, weights, radii or durations.

**Principles**

- **Quiet by default.** Near-black neutrals with a cool tint, one confident
  indigo accent, muted semantic tones. Chrome recedes; content reads first.
- **Two themes, one stylesheet.** Every color token is a CSS variable defined
  for dark (`:root`) and flipped by the `.light` class on `<html>`, driven by
  the theme preference (system / light / dark). Dark separates surfaces
  through brightness steps and hairlines; light adds soft shadows.
- **Tokens or nothing.** A raw hex, px, weight, or duration outside the token
  files is a bug. Component-internal geometry (a badge's dot offset) is the
  only exception and lives inside the component.
- **Motion is feedback.** Two speeds, no bounce, no decoration.

---

## Tokens

### Color — `src/styles.css` (variables) + `tailwind.config.ts` (names)

All colors are HSL triplets (`H S% L%`), consumed through `hsl(var(--…))`
Tailwind aliases. The token name in `tailwind.config.ts` *is* the class name
(`bg-panel-2`, `text-text-dim`, `bg-accent-soft`).

Surfaces stack from the window up to elevated controls:

| Token | Dark (`:root`) | Light (`.light`) | Used for |
|---|---|---|---|
| `bg` | `222 20% 5%` | `220 30% 97%` | window background |
| `sidebar` | `222 25% 7%` | `220 25% 94%` | the left navigation rail |
| `panel` | `222 20% 10%` | `0 0% 100%` | cards (the default surface) |
| `panel-2` | `222 20% 12%` | `220 25% 95%` | recessed regions: segmented track, secondary buttons |
| `inset` | `222 25% 6%` | `220 30% 94%` | text inputs (cut *into* the surface) |
| `hover` | `222 20% 15%` | `220 30% 92%` | hover wash on secondary/ghost controls |
| `raised` | `222 20% 17%` | `0 0% 100%` | floating elements: active segment |

| Token | Dark | Light | Used for |
|---|---|---|---|
| `border` | `222 20% 18%` | `220 20% 90%` | hairlines, card outlines |
| `border-strong` | `222 20% 23%` | `220 20% 85%` | control outlines, switch track |
| `text` | `220 15% 93%` | `220 20% 13%` | primary text |
| `text-dim` | `222 12% 65%` | `220 10% 40%` | secondary text, descriptions |
| `text-faint` | `222 10% 44%` | `220 10% 58%` | meta text, placeholders, disabled labels |
| `accent` | `231 100% 75%` | `231 75% 58%` | brand indigo — actions, selection, focus |
| `accent-soft` | 16% alpha | 10% alpha | accent-tinted surfaces (active nav, capability chips) |
| `on-accent` | `0 0% 100%` | `0 0% 100%` | text/icons on accent fill |
| `shadow` | transparent | `220 50% 8% / 0.06` | card elevation |

Semantic tones — `pass` `158 50% 52%`/`158 70% 36%`, `warn`
`40 70% 63%`/`40 80% 38%`, `fail` `0 70% 68%`/`0 70% 54%`, `skip`
`222 10% 47%`/`220 10% 65%`, `neutral` `222 10% 55%`/`220 10% 45%` — shared
by Badge, status dots and status-colored text. The raw status token
(`"pass"`, `"warn"`…) doubles as the component's tone key; i18n translates
only the display label.

### Typography — `tailwind.config.ts`

| Token | Size | Weight | Used for |
|---|---|---|---|
| `text-display` | 22 | bold | hero numbers, runtime status word |
| `text-title` | 17 | bold | page titles |
| `text-heading` | 14 | semibold | card section titles, preference rows |
| `text-body` | 13 | regular/medium | default reading, control labels |
| `text-small` | 12 | regular | secondary lines, details |
| `text-caption` | 11 | semibold | badges, table column headers, meta |
| `text-brand` | 15 | bold | sidebar wordmark only |

### Metrics — `tailwind.config.ts`

- Radius: `rounded-lg` 12 (cards) · `rounded-md` 8 (controls) · `rounded-sm`
  6 (segments, chips)
- Width: `w-sidebar` 200px (navigation rail)
- Shadows: `shadow-card`, `shadow-accent-glow` (primary buttons)
- Control heights: 20 (badges) · 26 (`size="sm"` buttons) · 30 (buttons,
  nav items) · 32 (inputs, dialogs)
- Icon sizes: 14 (in buttons) · 16 (nav, inline status) · 18 (list-row
  leading icons) · 32 (empty states)

### Motion

`transition-colors` is the default state transition (≈140ms); `transition-
transform` moves the Switch thumb. No bounces, no decorative animations.

---

## Components — `src/components/ui/`

| Component | Purpose | Key API |
|---|---|---|
| `Button` | the only button | `variant`: `primary`/`secondary`/`ghost`/`danger`; `size`: `default`/`sm`/`icon` |
| `Badge` | status pill | `variant` (tone vocabulary), `dot` (false → plain chip) |
| `Card` / `CardContent` | the page container | `className` (default padding is the caller's) |
| `Input` | single-line input | passes through to `<input>` |
| `Segmented` | 2–3 option picker | `options`, `value` (index), `onChange(index)`, `variant` `compact`/`full` |
| `Switch` | boolean toggle | `checked`, `onChange(checked)` |
| `Dialog` | modal editor shell | `open`, `title`, `onClose` |
| `DataList` / `DataListRow` / `EmptyState` | inventory tables and empty placeholders | `columns` (labels + widths), `primary`/`secondary`/`detail` |
| `PageBody` / `PageHeader` / `SectionHeader` | page scaffolding | `title`, `actions` |

**Tone vocabulary** (`pass | warn | fail | skip | accent | neutral`) is
shared by Badge, status dots and status text — one word drives color
everywhere.

## Shell & navigation

The desktop shell is a left navigation rail next to a single scrollable
content column — no per-page top bars and no page-to-page jumps.

- **Desktop (md 768px+):** a fixed `w-sidebar` 200px rail on the left; the
  content column to its right is the only scroll region. The rail stacks a
  brand row (52px, logo + wordmark), the primary nav (Overview, Plugins),
  and a footer area separated by a hairline that pins Settings with the
  adapter identity underneath.
- **Mobile (<md):** the rail is hidden and a compact top bar (52px) shows a
  menu button, brand, and the adapter identity. The menu button opens a left
  drawer (same 200px width, same navigation items) over a scrim; it closes
  on selection, Escape, or a scrim click.
- Navigation is local component state (`View = overview | plugins |
  settings`) — no router. `AppShell` renders the sidebar + the content column
  + the mobile nav; `NavItem` is the 32px nav row (active = `accent-soft`
  fill + accent text, idle = dim text + hover wash).

## Pages

The shell keeps navigation deliberately shallow: **three top-level pages** —
**Overview** (status + runtime controls + diagnostics + update banner),
**Plugins** (Installed / Market sub-views), and **Settings** (Configuration /
Preferences). Plugins was promoted out of Settings because its install /
update / remove / market workflow made the settings page too heavy; Settings
keeps only configuration (Profiles, Providers, Models, Snapshots) and
preferences (language, theme, start-at-login, close-to-tray, updates). Each
page hosts a small set of sub-views behind a `Segmented` control instead of
adding top-level navigation items.

## Page recipe

```tsx
<PageBody className="space-y-5">
  <PageHeader title={t("…")} actions={<Button … />} />
  <Card><CardContent>{/* hero / controls */}</CardContent></Card>
  <Segmented variant="full" options={[…] } value={section} onChange={…} />
  <SectionHeader title={t("…")} actions={<Button … />} />
  <DataList columns={[{ label: t("…") }, …]}>
    {items.map((item) => (
      <DataListRow key={item.id} primary={item.name} detail={item.id} … />
    ))}
  </DataList>
  <EmptyState icon={<Icon />}>{t("…")}</EmptyState>
</PageBody>
```

Inventory tables are a `DataList` with caption-bold column labels and body
rows (fixed leading columns, one flexible column, empty values render "—",
hover wash). The three inventory lists — Profiles, Providers, Models — reuse
the same components; the page only supplies column labels, widths and row
data. Below `lg` the detail column folds under the primary column.

## Iconography

[`lucide-react`](https://lucide.dev) stroke icons, sized 14–18px. Icons take
the text color of their context (`text-accent`, `text-text-dim`, …) — never
a raw hex.

## Responsive breakpoints

Tailwind breakpoints on the physical window width — no JavaScript breakpoint
logic:

| Tier | Breakpoint | Behavior |
|---|---|---|
| ≥ `md` (768px) | sidebar appears | the left rail (200px) replaces the mobile top bar and drawer; Overview stats stay 2-column |
| ≥ `lg` (1024px) | full data lists | the DataList detail column appears; Overview stats reflow to 4 columns |

Config tables never scroll horizontally — below `lg` the third (detail)
column folds under the primary column and the first column stretches to fill.

## Theming & i18n

The theme preference (`system | light | dark`) lives in the zustand store and
is applied by toggling the `light` class on `<html>` (dark is the default —
no class). `system` follows `prefers-color-scheme`.

All UI strings go through i18next with `t("…")` keys; the catalogs are
`src/locales/en.json` (source language) and `src/locales/zh.json`. When
adding a key, add it to both catalogs — the build does not fall back
silently, missing keys render as the raw key string.
