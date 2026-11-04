# CS-112 — Custom themes, shared themes, and a desktop window in the workspace's colours

**Wave:** 30 — make it yours (2026-11-04)
**Area:** frontend · desktop
**Blocked by:** CS-111
**Blocks:** —

## Problem

The built-in themes from CS-111 cover most people, but a team that wants its workspace in
its own brand colours has no way to get there. Slack offers a custom theme of four colours,
and a way to pass it to a colleague as text. On the desktop app, the rail and the title bar
follow light or dark only, so the window does not look like the workspace in front.

## Approach

- *Custom theme* in *Preferences → Appearance*: the four colours of CS-111, each with a
  colour picker and a hex field, previewed on the sidebar as they change.
- *Share* copies the theme as one line of four hex colours. *Import* takes that line, or a
  theme copied from Slack, which has the same shape. A line that is not four colours is
  refused with a message that says so.
- *Surprise me* picks four colours that pass the contrast rule of CS-111.
- The page already reports light or dark to the desktop app (`report_theme`). It reports the
  workspace's sidebar and badge colours too, and the app paints the rail and the title bar
  with them while that workspace is in front. A colour the app accepts is a hex colour and
  nothing else, as with icons in `workspaces.rs`.

## Acceptance

- [ ] A custom theme is saved like a built-in one and follows the person to other devices.
- [ ] Sharing a theme and importing it in another workspace gives the same four colours.
- [ ] Importing a line copied from Slack's *Share* works.
- [ ] In the desktop app, the rail and the title bar take the colours of the workspace in
  front, and change when another workspace comes to the front.

## Tests

Frontend: parsing a shared line, the Slack form included, and refusing a malformed one; the
contrast rule for *Surprise me*. Rust: `report_theme` refuses anything but hex colours.
