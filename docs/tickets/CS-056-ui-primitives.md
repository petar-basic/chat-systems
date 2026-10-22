# CS-056 — One set of controls: buttons, icon buttons, panels, dialogs

**Wave:** 18 — what a Slack user notices in the first hour
**Area:** frontend
**Blocked by:** —
**Blocks:** CS-057 (its panel is built from these pieces)

## Problem

Every screen styles its own controls, so nothing lines up across screens, and sometimes not
even inside one dialog. *Browse Channels* shows it in one place: its *Close* is the only button
there without `text-sm`, so it is 16 px among 14 px, and its `px-4` puts the text 16 px inside
the edge every row and the search field end on. The same hand-written pieces repeat everywhere:

| What | Where | What goes wrong |
|---|---|---|
| Primary button (`bg-purple-600 hover:bg-purple-500 …`) | 36 places | `text-sm` or not, `py-2` or `py-2.5`, `font-medium` or not, `disabled:` styles that differ |
| Secondary *Cancel* / *Close* | ~15 places | the *Browse Channels* problem in each of them |
| Right-panel header | 12 panels | the ✕ is a bare 16 px glyph with no padding — hard to hit on a phone; Slack's is a 28–32 px target |
| Icon-only button without `aria-label` | 10 panel close buttons (Pinned, Members, Settings, Audit log, Custom emoji, User groups, Integrations, Add instance, Profile, Channel members) | a screen reader says "button" |
| Spinner | 31 copies | three sizes and four colours |
| Titles | "Pinned Messages", "Create Channel", "Workspace Settings" next to "New message", "Files" | no rule; Slack uses sentence case |
| Closing a dialog | a footer *Close* in some, a ✕ in others, Escape everywhere | Slack: ✕ top-right in every dialog, actions bottom-right |

## Approach

### 1. The pieces, in `shared/components`

- **`Button`** — `variant`: `primary` · `secondary` · `ghost` · `danger`; `size`: `sm` (28 px)
  · `md` (36 px); `loading` (a spinner replaces the icon and the button is disabled);
  `icon` slot; `fullWidth`. One focus-visible ring, one disabled look. Text is always
  `text-sm`; height comes from the size, not from padding guesses.
- **`IconButton`** — 28 px (`sm`) or 32 px (`md`) hit area around a 16 px icon; `label` is a
  required prop, used for `aria-label` and the tooltip. Ghost by default; `pressed` for
  toggles (`aria-pressed`).
- **`Spinner`** — `sm` / `md`, current colour.
- **`PanelHeader`** — icon, title, optional actions, and the close `IconButton` labelled
  *Close {title}*. The 56 px height and the border every panel already uses.
- **`DialogHeader`** / **`DialogFooter`** — the title with a ✕ top-right, and right-aligned
  actions with an 8 px gap whose last button sits on the content's edge. `Modal` keeps
  focus trapping and Escape as today.

### 2. Every screen moves to them

All modals (*Browse channels*, *Create channel*, *New message*, *Channel settings*, *Forward*,
*Create workspace*, profile and settings dialogs, the huddle ring) and all twelve right panels,
plus the composer's and message actions' buttons where they are hand-styled. Footer *Close* /
*Cancel* buttons that only duplicate ✕ go; *Cancel* stays where it abandons typed input.

### 3. Rules that keep it that way

- **Sentence case** for every title and button label ("Browse channels", "Pinned messages").
- **An ESLint rule** (`no-restricted-syntax` on JSX `className` literals) refuses
  `bg-purple-600` / `hover:bg-purple-500` outside `shared/components`, and refuses a
  `<button>` whose only child is an icon unless it has `aria-label` — so the next screen cannot
  drift back.

## Acceptance

- [ ] No hand-written primary button, spinner or panel header outside `shared/components`;
      the lint rule fails the build if one comes back.
- [ ] Every icon-only control has an accessible name and at least a 28 px target.
- [ ] Every dialog has a ✕ top-right and right-aligned actions on the content edge; the
      *Browse channels* footer lines up with its rows.
- [ ] Titles and labels are sentence case.
- [ ] Nothing changes behaviour: existing unit and E2E tests pass with `data-qa` attributes
      preserved.

## Tests

Unit tests for `Button` (variants, `loading` disables and shows a spinner, `fullWidth`) and
`IconButton` (label becomes `aria-label` and title, `pressed` sets `aria-pressed`), and one
for `PanelHeader` closing. The existing component and E2E suites are the regression net for
the migration.
