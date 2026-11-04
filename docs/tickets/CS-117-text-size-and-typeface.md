# CS-117 — Text size and typeface

**Wave:** 30 — make it yours (2026-11-04)
**Area:** frontend · backend
**Blocked by:** CS-113
**Blocks:** —

## Problem

The app has one text size and one typeface, the system's sans. Someone who reads better
with larger text has only the browser's zoom, which the desktop app does not keep, and
someone who reads better with a typeface made for legibility has no way to choose one.

## Approach

- *Text size* in *Preferences → Appearance*: 90, 100, 112 and 125 %, set as the root font
  size so every `rem` follows. The arbitrary pixel sizes left in the frontend (about forty
  `text-[…px]`, `w-[…px]` and `h-[…px]` classes, and the pixel values in `index.css`) move
  to `rem` or to Tailwind's scale first, or they would stay the same size while the rest
  grows.
- *Typeface*: System, Inter, Atkinson Hyperlegible and OpenDyslexic. The three fonts are
  bundled with the frontend, since the content security policy allows fonts only from
  `'self'`, and each is loaded only when chosen. Code and code blocks keep the monospace.
- Both are kept in `users.preferences` from CS-113, as known keys with their defaults, so
  they follow the person to other browsers and the desktop app. Like the colour mode, the
  last value is also kept in the browser's storage and applied before the session arrives,
  so the page does not jump in size on start.
- The desktop app's own zoom (⌘+ / ⌘−) stays as it is; it is per window, this is per person.

## Acceptance

- [ ] At 125 %, the sidebar, messages, composer and dialogs grow together, and nothing is
  cut off or overlaps at 1280 × 800.
- [ ] Choosing a typeface changes the text everywhere except code, and loads only that font.
- [ ] Both settings are the same after signing in on another browser and in the desktop app.
- [ ] A size or typeface the server does not know is refused.

## Tests

Backend: defaults, a known size and typeface saved, an unknown value refused. Frontend: the
root font size for each step, the font loaded only when chosen. E2E: switch to 125 % and a
typeface, reload, and both are still applied.
