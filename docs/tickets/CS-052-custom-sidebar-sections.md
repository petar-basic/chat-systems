# CS-052 — Custom sidebar sections

**Wave:** 14 — the daily-use gaps a team arriving from Slack notices first
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

The sidebar has three fixed sections — channels, DMs, people — that can be collapsed, with
the collapsed state in `localStorage`
([`useCollapsedSections.ts`](../../frontend/src/features/channel/hooks/useCollapsedSections.ts)).
Somebody in forty channels cannot group *Projects*, *Clients* and *Social* the way every
Slack user does, and their layout does not follow them to another device.

## Approach

1. `sidebar_sections (id, user_id, workspace_id, name, position, collapsed)` and a nullable
   `channel_members.section_id`; channels with none stay in the default *Channels* section.
   Per user, per workspace, invisible to everybody else.
2. CRUD under `/workspaces/{ws_id}/sidebar/sections`, plus moving a channel into a section
   and reordering sections. Deleting a section returns its channels to the default.
3. Collapsed state moves to the server for custom and built-in sections alike, so the layout
   follows the person across devices; `localStorage` stays only as the first paint.
4. Frontend: *Create section* in the sidebar menu, *Move to…* on a channel's context menu
   and drag-and-drop between sections, rename and delete on the section header. Keyboard
   reachable (the drag has a menu equivalent).

## Acceptance

- [ ] A member can create, rename, reorder and delete sections and move channels between them.
- [ ] The layout is identical on a second device after sign-in.
- [ ] Leaving a channel removes it from its section; rejoining puts it in the default.
- [ ] Nobody else's sidebar changes.

## Tests

`http_tests` for CRUD, ownership (one member cannot touch another's sections) and the delete
fallback. A frontend test for move-by-menu.
