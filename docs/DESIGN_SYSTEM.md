# DeepMate Desktop Design System

The design system for the DeepMate desktop shell (Tauri 2 + React +
Tailwind v4 + shadcn/ui): tokens, components, page patterns, and the rules
that keep the UI consistent. Every visual value must come from the design
tokens in `src/styles.css`; pages and components never invent their own
colors, sizes, weights, radii or durations.

**Principles**

- **Quiet by default.** Near-black neutrals with a cool tint, a Notion-style
  monochrome interaction color, muted semantic tones. Chrome recedes; content
  reads first.
- **Two themes, one stylesheet.** Every color token is a CSS variable defined
  for dark (`:root`) and flipped by the `.light` class on `<html>`, driven by
  the theme preference (system / light / dark). Dark separates surfaces
  through brightness steps and hairlines; light adds soft shadows.
- **Tokens or nothing.** A raw hex, px, weight, or duration outside the token
  file is a bug. Component-internal geometry (a badge's dot offset) is the
  only exception and lives inside the component.
- **Motion is feedback.** Two speeds, no bounce, no decoration.

---

## Tokens

### Color — `src/styles.css` (variables + `@theme`)

All colors are HSL triplets (`H S% L%`), consumed through `hsl(var(--…))`
Tailwind aliases. The token name in the `@theme` block *is* the class name
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
| `accent` | `0 0% 96%` | `0 0% 13%` | interaction color — actions, selection, focus. Monochrome *inversion*: near-white fill on dark, near-black on light, like Notion's buttons |
| `accent-soft` | `222 20% 15%` | `220 25% 90%` | gray wash for selected surfaces (active nav, filter chips) |
| `on-accent` | `0 0% 0%` | `0 0% 100%` | text/icons on accent fill (the inverted partner) |
| `shadow` | transparent | `220 50% 8% / 0.06` | card elevation |

Semantic tones — `pass` `158 50% 52%`/`158 70% 36%`, `warn`
`40 70% 63%`/`40 80% 38%`, `fail` `0 70% 68%`/`0 70% 54%`, `skip`
`222 10% 47%`/`220 10% 65%`, `neutral` `222 10% 55%`/`220 10% 45%` — shared
by Badge, status dots and status-colored text. The raw status token
(`"pass"`, `"warn"`…) doubles as the component's tone key; i18n translates
only the display label.

### Typography — `@theme` in `src/styles.css`

| Token | Size | Weight | Used for |
|---|---|---|---|
| `text-display` | 22 | bold | hero numbers, runtime status word |
| `text-title` | 17 | bold | page titles |
| `text-heading` | 14 | semibold | card section titles, preference rows |
| `text-body` | 13 | regular/medium | default reading, control labels |
| `text-small` | 12 | regular | secondary lines, details |
| `text-caption` | 11 | semibold | badges, table column headers, meta |
| `text-brand` | 15 | bold | sidebar wordmark only |

### Metrics — `@theme` in `src/styles.css`

- Radius: `rounded-lg` 12 (cards) · `rounded-md` 8 (controls) · `rounded-sm`
  6 (segments, chips)
- Width: `w-sidebar` 200px (navigation rail) · `max-w-content` 1000px (page
  column cap)
- Shadows: `shadow-card`
- Control heights: 20 (badges) · 26 (`size="sm"` buttons) · 30 (buttons,
  nav items) · 32 (inputs, dialogs)
- Icon sizes: 14 (in buttons) · 16 (nav, inline status) · 18 (list-row
  leading icons) · 32 (empty states)

### Motion

Two speeds, no bounce, no decoration. CSS state feedback uses the default
`transition-colors` (~140ms). JS-driven motion — the Motion library
(`motion/react`) — uses the tokens in `src/lib/motion.ts`: `MOTION_DURATION.fast`
(140ms) for list reflow and page exits, `MOTION_DURATION.base` (180ms) for page
and section entrances, plus the shared `MOTION_EASE` bezier curves and
`STAGGER` (35ms). Durations and easings are design tokens: never inline a raw
number in a component.

Page switches animate through `AnimatePresence` in `AppShell` (directional
slide + fade, `mode="wait"`, scroll resets between views); the active nav item
shares a `layoutId` so the highlight pill slides between items; plugin lists
use `layout` + `AnimatePresence` so rows fade and reflow when the filter
changes; Overview sections stagger in with `fadeUp`.
`MotionConfig reducedMotion="user"` (root of `App.tsx`) honors
`prefers-reduced-motion`: transform and layout animations are skipped, opacity
kept — matching the CSS fallback in `styles.css`.

---

## Components — `src/components/ui/`

Generated by the shadcn CLI (Radix primitives) and adapted onto the DeepMate
tokens. The shadcn variable surface (`--primary`, `--background`, …) maps to
the DeepMate tokens in `styles.css`; the brand `--accent` keeps its DeepMate
meaning, so generated components use `bg-hover` for hover highlights instead
of shadcn's `--accent` role. `dark:` variants are stripped — theming is the
`.light` class flip.

| Component | Purpose | Key API |
|---|---|---|
| `Button` | the only button | `variant`: `primary`/`secondary`/`ghost`/`danger`; `size`: `default`/`sm`/`icon` |
| `Badge` | status pill | `variant` (tone vocabulary), `dot` (false → plain chip) |
| `Card` / `CardContent` | the page container | `className` (default padding is the caller's) |
| `Input` / `Textarea` | text entry | pass through to the native element |
| `Label` | form field caption | Radix label |
| `Select` | option picker | Radix select (replaces the old Segmented for preferences) |
| `Tabs` | 2–3 option picker | Radix tabs, styled like the old Segmented (`compact`/`full` variants) |
| `Switch` | boolean toggle | Radix switch, `checked`/`onCheckedChange` |
| `Dialog` | modal editor shell | Radix dialog (focus trap, Esc, scroll lock) |
| `AlertDialog` / `ConfirmDialog` | destructive confirmations | Radix alert dialog; `ConfirmDialog` wraps it with i18n labels |
| `Table` | inventory tables | semantic table (replaces DataList) |
| `Sheet` | mobile nav drawer | Radix dialog side sheet |
| `Tooltip` | hover hints | Radix tooltip |
| `Toaster` | toast host | sonner, themed by the CSS variables |
| `Skeleton` | loading placeholders | pulse block |
| `EmptyState` | empty placeholders | `icon`, `children`, optional `action` CTA |
| `PageBody` / `PageHeader` / `SectionHeader` | page scaffolding | `title`, `actions`; `PageBody` caps width at `max-w-content` |

**Tone vocabulary** (`pass | warn | fail | skip | accent | neutral`) is
shared by Badge, status dots and status text — one word drives color
everywhere.

## Shell & navigation

The desktop shell is a left navigation rail next to a single scrollable
content column — no per-page top bars and no page-to-page jumps.

- **Desktop (md 768px+):** a fixed `w-sidebar` 200px rail on the left; the
  content column to its right is the only scroll region. The rail stacks a
  brand row (52px, logo + wordmark) above a single scrollable nav column
  with two labeled groups: **工作区 Workspace** (Overview, Plugins) and
  **设置 Settings** (Profiles / Models / Snapshots / Preferences / About).
- **Mobile (<md):** the rail is hidden and a compact top bar (52px) shows a
  menu button and the brand. The menu button opens a left drawer (same
  200px width, same navigation items) over a scrim; it closes on selection,
  Escape, or a scrim click.
- Navigation is local component state (`View = overview | plugins | profiles
  | models | snapshots | preferences | about`) — no router.
  `AppShell` renders the sidebar + the content column + the mobile nav;
  `NavItem` is the 32px nav row (active = `accent-soft` gray wash + full text
  color, idle = dim text + hover wash).

## Pages

The shell keeps navigation deliberately shallow: **all pages sit at the top
level** — **Overview** (status + runtime controls + diagnostics + update
banner), **Plugins** (Installed / Market sub-views), and **five configuration
pages** (Profiles / Models / Snapshots / Preferences / About)
grouped under the 设置 heading in the sidebar. Settings was flattened out of
a two-level nav because the sidebar had room for every section directly;
Plugins was promoted out of Settings for the same reason — its install /
update / remove / market workflow made the settings page too heavy.

The **Models** page merges the previous Providers and Models pages into one
full-height split panel: the left column is a provider sidebar grouped into
默认 (the built-in DeepSeek route) and 自定义供应商, ending in an
add-provider row; the right column edits the selected provider **in place**
— Base URL, wire-protocol select (`openai-responses` /
`openai-completions`, plus the harness default) and the API key env var
masked behind a reveal toggle (the key itself is never stored, only the env
var name) — and lists its models as rows: model id in mono, context-window
badge (`1M` / `262.1K`), vision badge when `input` includes `image`, and
per-row edit/delete. The provider panel height tracks the viewport
(`md:h-[calc(100dvh-160px)]`) and each column scrolls independently. Model
edits happen in a compact dialog (model id, context window, max output,
input modality chips with "text" locked on) — no raw JSON fields; advanced
harness flags stay YAML-only. Deleting a provider removes its models
together. Below md the two columns stack.

Each configuration page is a single section behind a `PageHeader` (title +
create actions where a single action fits; the split Models page keeps its
actions inside the layout): one heading, no tabs, no sub-nav.

**Page width.** Every page column is fluid, capped at `max-w-content`
(1000px), and **centered** past the cap (`mx-auto`) — resizing the window
keeps the column centered instead of pinning it to the left edge.

## Page recipe

```tsx
<PageBody className="space-y-5">
  <PageHeader title={t("…")} actions={<Button … />} />
  <Card><CardContent>{/* hero / controls */}</CardContent></Card>
  <Tabs value={tab} onValueChange={setTab}>
    <TabsList><TabsTrigger value="a">…</TabsTrigger></TabsList>
  </Tabs>
  <SectionHeader title={t("…")} actions={<Button … />} />
  <Card className="overflow-hidden">
    <Table>
      <TableHeader><TableRow><TableHead>…</TableHead></TableRow></TableHeader>
      <TableBody>{items.map((item) => <TableRow key={item.id}>…</TableRow>)}</TableBody>
    </Table>
  </Card>
  <EmptyState icon={<Icon />}>{t("…")}</EmptyState>
</PageBody>
```

Inventory tables are a `Table` with caption-bold column labels and body rows
(fixed leading columns, one flexible column, empty values render "—", hover
wash). The three inventory lists — Profiles, Providers, Models — reuse the
same components; the page only supplies column labels, widths and row data.
Below `lg` the detail column folds under the primary column.

**Feedback.** Mutating actions confirm through `ConfirmDialog` (delete,
import); every command failure surfaces as a toast with a friendly message
(`lib/errors.ts` maps known failure patterns); successful mutations toast
their outcome. The `busyAction` store field tracks the running operation —
read-only operations (loads, search, doctor) never block the rest of the UI,
mutating operations block each other.

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
