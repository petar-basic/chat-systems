# CS-059 — Dialogs that look finished

**Wave:** 20 — dialogs and large calls
**Area:** frontend
**Blocked by:** —
**Blocks:** —

## Problem

CS-056 gave every dialog the same buttons, header and close button. Looked at side by side
(2026-10-23, 1280×800, light and dark) they still read as five different hands:

- **Widths.** Each dialog picks its own: *Create channel* and *New message* `max-w-sm`,
  *Channel settings*, *Forward* and *Profile* `max-w-md`, *Browse channels* and *Keyboard
  shortcuts* `max-w-lg`. Opening one after another, the frame jumps.
- **Structure.** `Modal` scrolls the whole dialog, so in a long one the title and the
  buttons scroll away. *Profile* works around it with a bordered header, its own scroll area
  (with a visible scrollbar) and a sticky footer; nothing else does.
- **Fields.** 67 `<input>`s carry 19 different class strings in two heights (`py-2.5 text-sm`
  and `py-3 text-base`) that sit next to each other: *Create channel*'s name is taller than its
  description, *New message*'s search is shorter than both. *Create channel* has no label on
  the name, only a placeholder, while *Channel settings* labels every field. The two native
  checkboxes and six native `<select>`s look like the operating system, not the app.
- **Confirmations.** Deleting a message, archiving a channel and deleting a hook each build
  their own inline "are you sure" row. Slack asks in a small dialog that shows what is about to
  go (the message itself, the channel name) with a red action.
- **Profile.** One dialog holds the profile, the theme, email notifications and two-factor,
  with two save buttons (*Set status* and *Save profile*). Slack has *Edit profile* (fields on
  the left, photo on the right) and *Preferences* (a left-hand list: Notifications, Appearance,
  Security…), each saving as you go or once.
- **Browse channels** is a short list in a mid-size box. Each row mixes three button styles
  (*Join* primary, *Open* grey, *Leave* plain text); there is no sort, no count and no way to
  create a channel from there. Slack's is a large dialog with a search, a sort, the count, and
  the row's action on hover.
- **Motion.** Dialogs appear and vanish in one frame; Slack fades and scales them in about
  150 ms.

## Approach

Three parts, each its own PR, in this order.

### 1. The pieces

- **`Modal` anatomy**: `size` (`sm` 400 px, `md` 520 px, `lg` 720 px), a fixed header
  (`DialogHeader`), a body that is the only thing that scrolls (a hairline above and below it
  only while it does), and a fixed footer (`DialogFooter`). 24 px padding everywhere; the phone
  sheet keeps working as it does now.
- **Motion**: fade the backdrop, fade and scale the dialog from 98 %, 150 ms in and 100 ms
  out; none under `prefers-reduced-motion`.
- **Form controls** in `shared/components/Form`: `Field` (label, optional hint, error, and a
  character counter when given a limit, wired with `htmlFor` and `aria-describedby`),
  `TextInput`, `TextArea`, `Select`, `Checkbox`, `RadioCards` (the Public / Private choice). One
  height (36 px) and one text size (14 px) for every field; the composer is out of scope.
- **`ConfirmDialog`**: title, what goes (a slot for a preview), a danger or primary action,
  Enter confirms and Esc cancels.
- **Lint**: the CS-056 `no-restricted-syntax` pattern grows to refuse a hand-styled
  `<input>`, `<textarea>` or `<select>` outside `shared/components/Form`.

### 2. Every dialog

- *Create channel*: `RadioCards` for public / private, the name with a `#` prefix, a label and
  the 80-character counter the API enforces; spaces become hyphens and capitals lower case as
  you type, as Slack does (in the client only — existing names are left alone, and the API
  keeps accepting what it accepts today); *Announcement channel* as a `Checkbox` with its
  hint.
- *New message*, *Channel settings*, *Forward*, *Keyboard shortcuts*: the new sizes, fields and
  footer.
- *Browse channels*: `lg`; the count, a sort (name, most members, newest), *Create channel* in
  the header, and one action per row in one style, shown on hover and on focus.
- The three inline confirmations become `ConfirmDialog`s; deleting a message shows the message.

### 3. Profile and Preferences

- *Edit profile*: display name, status (one save for the dialog, with the presets and *clear
  after*), bio, and the photo on the right.
- *Preferences*: a left-hand list — *Notifications* (email digest, push devices), *Appearance*
  (theme), *Security* (two-factor) — showing one section at a time, each saving as it changes.
- *You* menu: *Profile* and *Preferences* as two entries.

## Acceptance

- [ ] Every dialog uses one of three widths, and in a long one the title and the buttons stay
      put while the body scrolls.
- [ ] Every field in a dialog has a label, one height and one text size; no native checkbox or
      select is left in a dialog.
- [ ] Deleting a message, archiving a channel and deleting a hook ask in a `ConfirmDialog`.
- [ ] *Create channel* chooses public / private with cards and shows the name as it will be.
- [ ] *Browse channels* is large, sorts, counts, and offers one action per row.
- [ ] Profile and preferences are two dialogs, each with one way to save.
- [ ] Dialogs fade in and out, and do not under reduced motion.
- [ ] Light, dark and the phone sheet are checked for every dialog.

## Tests

Component tests for `Modal` (only the body scrolls, focus is trapped and returned, Esc and the
backdrop close), `Field` (label and description are associated, the counter counts, the error
is announced), `RadioCards` (arrow keys move the choice) and `ConfirmDialog` (Enter confirms,
Esc cancels). Existing dialog tests and E2E specs keep passing with their `data-qa` hooks; the
E2E suite gains *create a private channel from the cards* and *delete a message through the
confirmation*. A manual pass over every dialog in light, dark and at a phone width goes in
`docs/manual-qa.md`.
