# CS-069 — Small UI fixes and a README that says what ships

**Wave:** 24 — ready for a company
**Area:** frontend · backend · docs
**Blocked by:** —
**Blocks:** —

## Problem

The 2026-10-26 walk through the app found small things a new team sees in its first hour:

- The channel header has *Channel members* twice: the member count and an icon beside it
  ([`ChannelHeader.tsx`](../../frontend/src/features/channel/ChannelHeader.tsx)), among
  seven unlabeled icons.
- The workspace rail shows one letter on a grey tile, so three workspaces starting with D
  are three identical "D" tiles; only the uploaded icon tells them apart.
- A workflow's form summary is a stack of **Label** / answer paragraphs, so a five-question
  form is a message taller than the screen. Slack lays the same answers out as fields in two
  columns.
- The README's *Features* list stops before Wave 21: no notification levels, keywords or
  schedule, no file search, polls, lists or workflows. It is the first page a company reads.

## Approach

One PR.

- **Header.** The count opens the member panel; the separate members icon goes. Files and
  pinned messages stay; anything a member uses less than weekly moves into *More channel
  actions*. Every icon keeps a tooltip.
- **Rail.** A workspace without an icon gets a tile coloured from its id (the avatar palette
  people already have) and up to two letters (`Dev Team` → `DT`), so two workspaces differ
  even with the same initial. The full name stays the tooltip and accessible name.
- **Form summary as fields.** The summary step stores its answers as `metadata.fields`
  (`[{ label, value }]`, the value already rendered: a person as a mention) beside the
  Markdown it posts today, which stays the content so search, notifications and other
  clients keep working. The renderer shows `metadata.fields` as a two-column grid (one
  column on a phone); a long answer spans both.
- **README.** *Features* and *Coming from Slack* list what ships as of 3.3.0, grouped the way
  the app is (messaging, notifications, finding things, structured work, admin and
  compliance), each line short.

## Acceptance

- [ ] The channel header has one way to open members.
- [ ] Two workspaces with the same first letter look different in the rail.
- [ ] A form summary renders as a field grid, and its content is unchanged.
- [ ] The README names every shipped wave's user-facing feature.

## Tests

Frontend: the header has one members control; the rail tile's letters and colour for a
name; `metadata.fields` renders as a grid and falls back to the content without it. Backend:
the summary step writes `metadata.fields` with the same answers as the Markdown.
