# CS-111 — A theme for each workspace

**Wave:** 30 — make it yours (2026-11-04)
**Area:** frontend · backend
**Blocked by:** —
**Blocks:** CS-112

## Problem

Every workspace looks the same. Someone in three workspaces tells them apart by the name at
the top of the sidebar, and on the desktop rail by a small tile. Slack lets each person give
each workspace its own sidebar colours, and a team arriving from it looks for that in
*Preferences → Appearance*. Light, dark and system is all there is today, and the sidebar
uses the same tokens as the rest of the app, so it cannot be coloured on its own.

## Approach

- The sidebar gets tokens of its own: `--sidebar`, `--sidebar-fg`, `--sidebar-muted`,
  `--sidebar-selected`, `--sidebar-selected-fg`, `--presence` and `--badge`. By default they
  resolve to today's colours, so nothing changes until someone picks a theme.
- A theme is four colours, as in Slack: the sidebar, the selected item, the presence dot and
  the notification badge. The text colours are derived from them so the sidebar stays
  readable; a colour pair below WCAG AA contrast is adjusted, not refused.
- A handful of built-in themes, the current look first and named as the default. A theme
  colours the sidebar and the workspace's own chrome only. Messages, threads and dialogs
  keep following light, dark or system, which stays one setting for the whole app.
- A member's theme is stored per workspace on the server, in a `theme` column on
  `workspace_members` next to the profile and notification columns, so it follows them to
  every browser and to the desktop app. It is the member's own: nobody else sees it, and an
  admin cannot set it for them.
- *Preferences → Appearance* shows the built-in themes under the colour mode, says that the
  theme is for this workspace only, and applies a choice at once.

## Acceptance

- [ ] Picking a theme recolours the sidebar of that workspace at once, and switching to
  another workspace shows that one's theme.
- [ ] The choice is there after signing in on another browser.
- [ ] Light, dark and system still switch messages, threads and dialogs, whatever the theme.
- [ ] Every built-in theme passes AA contrast for the sidebar's text, in light and dark mode.

## Tests

Backend: the theme is saved for one member of one workspace and refused when malformed.
Frontend: the tokens follow the current workspace; the contrast check over every built-in
theme. E2E: pick a theme, reload, it is still there.
