# DeepMate Desktop Design System

The design system for the DeepMate desktop shell: tokens, components, page
patterns, and the Slint-specific rules that keep the UI consistent. Every
visual value must come from `ui/theme/`; pages and components never invent
their own colors, sizes, weights, or durations.

**Principles**

- **Quiet by default.** Near-black neutrals with a cool tint, one confident
  indigo accent, muted semantic tones. Chrome recedes; content reads first.
- **Two themes, one file.** Every color token switches on `Colors.dark`,
  driven by the Rust host (system / light / dark). Dark separates surfaces
  through brightness steps and hairlines; light adds soft shadows.
- **Tokens or nothing.** A raw hex, px, weight, or duration outside
  `ui/theme/` is a bug. Component-internal geometry (a badge's dot offset)
  is the only exception and lives inside the component.
- **Motion is feedback.** Two speeds, no bounce, no decoration.

---

## Tokens

### Color — `ui/theme/Colors.slint`

Surfaces stack from the window up to elevated controls:

| Token | Dark | Light | Used for |
|---|---|---|---|
| `bg` | `#0b0d11` | `#f4f6f9` | window background |
| `sidebar` | `#0e1117` | `#eceff4` | the left navigation rail |
| `panel` | `#14171e` | `#ffffff` | cards (the default surface) |
| `panel-2` | `#191d26` | `#f0f2f6` | recessed regions: table headers, segmented track, disabled primary |
| `inset` | `#0d1015` | `#edf0f4` | text inputs (cut *into* the surface) |
| `hover` | `#1f242f` | `#e6eaf0` | hover wash on secondary/ghost controls |
| `raised` | `#232936` | `#ffffff` | floating elements: active segment |

| Token | Dark | Light | Used for |
|---|---|---|---|
| `border` | `#242a36` | `#e2e6ec` | hairlines, card outlines |
| `border-strong` | `#303847` | `#d2d8e1` | hovered control outline |
| `text` | `#e9ebf0` | `#1b1f27` | primary text |
| `text-dim` | `#9ba3b4` | `#5d6673` | secondary text, descriptions |
| `text-faint` | `#636c7c` | `#8b94a1` | meta text, placeholders, disabled labels |
| `accent` | `#7d93ff` | `#4c63e7` | brand indigo — actions, selection, focus |
| `accent-soft` | 16% | 10% alpha | accent-tinted surfaces (active nav, capability chips) |
| `on-accent` | `#ffffff` | `#ffffff` | text/icons on accent fill |
| `shadow` | transparent | `#101828` @ 6% | card/segment/secondary-button elevation |
| `focus-ring` | 30% | 22% accent alpha | halo behind focused inputs |

Semantic tones — `pass` `#45c08d`/`#1c9e6c`, `warn` `#e2b45f`/`#b07d14`,
`fail` `#e86e70`/`#d04444`, `skip` `#6b7484`/`#99a1ad`, `neutral`
`#8b94a3`/`#6b7280` — shared by Badge, StatusDot, and status-colored text.
The raw status token (`"pass"`, `"warn"`…) doubles as the component's `kind`
/`tone` key; i18n translates only the display label (see `src/i18n.rs`).

### Typography — `ui/theme/Typography.slint`

| Token | Size | Weight pairing | Used for |
|---|---|---|---|
| `display` | 22 | bold | hero numbers, runtime status word |
| `title` | 17 | bold | page titles |
| `heading` | 14 | semibold | card section titles |
| `body` | 13 | regular/medium | default reading, control labels |
| `small` | 12 | regular | secondary lines, details |
| `caption` | 11 | semibold/bold | badges, table column headers, meta |
| `brand` | 15 | bold | top-nav wordmark only |

Weights: `regular` 400 · `medium` 500 (actionable labels) · `semibold` 600
(in-content emphasis) · `bold` 700 (titles, numbers). Never use raw numbers.

### Metrics — `ui/theme/Metrics.slint`

- Radius: `radius-lg` 12 (cards) · `radius` 8 (controls) · `radius-sm` 6 (segments, chips)
- Spacing: `xs` 4 · `sm` 8 · `md` 12 · `lg` 16 · `xl` 20 · `xxl` 28
- Layout: `page-padding` 24 · `content-max` 900 · `topnav-height` 52 (the
  sidebar brand row and the mobile compact top bar share this height) ·
  `sidebar-width` 200
- Control heights: `control-xs` 20 (badges) · `control-sm` 30 (buttons,
  segmented) · `control` 32 (inputs, nav items) · `row-header` 36 · `row` 46
- Icon sizes: `icon-sm` 14 (in buttons/inputs/banners) · `icon-md` 16 (nav,
  inline status) · `icon-lg` 18 (list-row leading icons, tile chips) ·
  `icon-xl` 26 (empty states)

### Motion — `ui/theme/Motion.slint`

`fast` 100ms (hover reveals) · `base` 140ms (color/state transitions).

---

## Components — `ui/components/`

| Component | Purpose | Key API |
|---|---|---|
| `Icon` | theme-tinted SVG glyph | `source`, `tone`, `size` (default `icon-md`) |
| `DmButton` | the only button | `text`, `icon`/`has-icon`, `kind`: `primary`/`secondary`/`ghost`/`danger`, `enabled`, `clicked()` |
| `TextField` | single-line input | `text` (two-way), `placeholder`, `icon`/`has-icon`, `accepted()`, `edited(string)` |
| `Segmented` | 2–3 option picker | `options`, `current` (two-way), `selected(int)` |
| `Badge` | status pill | `text`, `kind` (tone vocabulary), `show-dot` (false → plain chip) |
| `StatusDot` | glowing status dot | `tone`, `size` |
| `BusySpinner` | stepped loader | `tone`, `size` |
| `Card` | the page container | `inner-padding` (default `lg`), children stack vertically |
| `ListPanel` | framed row list | children manage own padding; hairlines between rows |
| `TableHeader` | three-column table header | `col-1`/`col-2`/`col-3`, `col-1-width`, `col-2-width` |
| `TableRow` | one three-column inventory row | `col-1`/`col-2`/`col-3`, widths, `show-divider`; hover wash |
| `StatTile` | inventory metric | `label`, `value` (<0 → "—"), `icon` |
| `EmptyState` | empty list placeholder | `icon`, `message`, `hint` |
| `PageHeader` | title + subtitle + actions slot | `title`, `subtitle`; children = right-aligned actions |
| `PageBody` | scrolling, max-width column | children stack with `lg` spacing |
| `TopNavbar` | top navigation bar (superseded by the React sidebar shell; kept for the Slint app) | `items`, `selected`, `activated(int)`; brand + nav tabs + adapter identity |

**Tone vocabulary** (`pass | warn | fail | skip | accent | neutral`) is shared
by Badge, StatusDot, and status text — one word drives color everywhere.

## Shell & navigation (React)

The desktop shell is a left navigation rail next to a single scrollable
content column — no per-page top bars and no page-to-page jumps.

- **Desktop (md 768px+):** a fixed `sidebar-width` 200px rail on the left; the
  content column to its right is the only scroll region. The rail stacks a
  brand row (`topnav-height` 52, logo + wordmark), the primary nav (Overview,
  Plugins), and a footer area separated by a hairline that pins Settings with
  the adapter identity underneath.
- **Mobile (<md):** the rail is hidden and a compact top bar (`topnav-height`
  52) shows a menu button, brand, and the adapter identity. The menu button
  opens a left drawer (same `sidebar-width`, same navigation items) over a
  scrim; it closes on selection, Escape, or a scrim click.
- Navigation is local component state (`View = overview | plugins |
  settings`) — no router. `AppShell` renders `AppSidebar` + the content column
  + `MobileNav`; `SidebarNav` is the navigation list shared by the rail and
  the drawer, and `NavItem` is the 32px nav row (active = `accent-soft` fill +
  accent text, idle = dim text + hover wash).

## Pages

The shell keeps navigation deliberately shallow: **three top-level pages** —
**Overview** (status + runtime controls + diagnostics), **Plugins** (Installed
/ Market sub-views), and **Settings** (Configuration / Preferences). Plugins
was promoted out of Settings because its install / update / remove / market
workflow made the settings page too heavy; Settings keeps only configuration
and preferences. Each page hosts a small set of sub-views behind a `Segmented`
control instead of adding top-level navigation items.

## Page recipe

```
PageHeader { title: …; subtitle: …; /* actions */ }
PageBody {
    Card { /* hero / controls */ }
    Segmented { /* sub-view switch, when the page has more than one view */ }
    Card { /* install / search bar */ }
    ListPanel {
        TableHeader { col-1: …; col-2: …; col-3: … }
        for item: TableRow { col-1: …; col-2: …; col-3: …; show-divider: index > 0 }
    }
    EmptyState { /* when the model is empty */ }
}
```

Inventory tables are a `ListPanel` with a `TableHeader` row (caption bold faint
column labels) and `TableRow` body rows (fixed leading columns, one flexible
column, empty values render "—", and a `Colors.hover` wash on hover). Three
inventory lists — Profiles, Providers, Models — reuse the same components; the
page only supplies the column labels, widths, and row data.

## Iconography — `ui/assets/icons/`

Feather-style stroke icons: 24×24 viewBox, `stroke="#ffffff"`,
stroke-width 2, round caps/joins, monochrome. `Icon` recolors via
`colorize`, so assets stay white; tint comes from the theme. Filled
variants (play, stop) use `fill="#ffffff"`.

## Slint implementation rules

Hard-won constraints — violating any of these produces a compile error or a
silent layout bug:

1. **Icon + text rows are positioned by hand.** A BoxLayout does not center
   an `Image` against a `Text` on its cross axis (the icon rides high).
   Pattern: icon gets `y: (parent.height - self.height) / 2`; text gets
   `height: 100%; vertical-alignment: center`. See `DmButton`, `NavItem`.
2. **`if` cannot be a `for` loop body.** Wrap the item in a layout and put
   the conditional inside it (see `TopNavbar`'s brand block).
3. **A layout child with explicit `x`/`width` stops propagating
   `preferred-height` to its parent.** Set `height: child.preferred-height`
   on the container explicitly (see `PageHeader`).
4. **Texts absorb distributed space in BoxLayouts.** Pin structure with
   `vertical-stretch: 0` on headers and `alignment: start` on stacked
   content, or spacing lands between your texts.
5. **`padding` is reserved** on components — expose `inner-padding` instead.
6. **Nested layouts that bind `width: Math.min(root.width, …)` create
   binding loops.** Center with manual `x: (root.width - self.width) / 2`.
7. **Breakpoints must not read `parent.width`.** A binding on a layout's own
   width forms a `layoutinfo` cycle. Feed the physical window width from Rust
   into an `in property` (`AppWindow.viewport-width`, updated by
   `wire_viewport_follow` in `main.rs`) and drive `compact`/`narrow` from
   that. `GridLayout` has no `columns` property — the column count comes from
   the children's `col`/`row` indices, so render 2- vs 4-column variants as
   two `if` branches.

## Responsive breakpoints

The React shell uses Tailwind breakpoints on the physical window width — no
JavaScript breakpoint logic:

| Tier | Breakpoint | Behavior |
|---|---|---|
| ≥ `md` (768px) | sidebar appears | the left rail (`sidebar-width` 200) replaces the mobile top bar and drawer; Overview stats stay 2-column |
| ≥ `lg` (1024px) | full data lists | the DataList detail column appears; `compact` table columns show; Overview stats reflow to 4 columns |

Config tables never scroll horizontally — below `lg` the third (detail)
column folds under the primary column and the first column stretches to fill.

(The Slint app keeps its own window-width tiers — wide ≥1100px, medium
820–1100px, compact <820px — driven from Rust as described in the
implementation rules above.)

## Theming & i18n

`Colors.dark` is set once by the Rust host from the resolved theme. All UI
strings use `@tr` and live in `translations/<lang>/LC_MESSAGES/
deepmate-desktop.po`; nav labels and status/display labels flow through
`src/i18n.rs` because they are models, not markup. When adding a `@tr`
string, add its msgid to every catalog — the build uses
`DefaultTranslationContext::None`, so msgids must match the source text
byte-for-byte.
