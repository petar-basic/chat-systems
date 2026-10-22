# CS-057 — Later: saved messages and reminders as one to-do list

**Wave:** 18 — what a Slack user notices in the first hour
**Area:** backend · frontend
**Blocked by:** CS-056 (the panel and its controls)
**Blocks:** —

## Problem

Slack's *Later* is where people keep what they still have to do: save a message and it is an
item *In progress*; give it a reminder and it comes back when due; tick it and it moves to
*Completed*; or put it away in *Archived*. Reminders made with `/remind` live in the same
list. We have the pieces, apart and without a way to finish anything:

- **Saved items** (`saved_messages`: message, optional note) — a list with no status; the
  only way to say "done" is to delete it.
- **Reminders** (`reminders` in the hooks module: text, optional message, `remind_at`,
  `is_delivered`) — a second panel; a delivered reminder just disappears from view.

## Approach

### 1. One table, one list

`later_items (id, workspace_id, user_id, created_by, message_id?, channel_id?, text?, note?,
status, remind_at?, reminded_at?, completed_at?, created_at, updated_at)` with `status` in
`in_progress · completed · archived`, keyed to the workspace membership (leaving the workspace
takes them along) and with a partial index on `remind_at` for undelivered reminders.

Migration: every saved message becomes an *in progress* item with its note; every reminder
becomes an item with its text, message and `remind_at` — delivered ones keep `reminded_at`
and stay *in progress*, as Slack keeps a fired reminder until it is ticked. A reminder somebody
set for someone else keeps `created_by`. `saved_messages` and `reminders` are dropped; the
saved and reminders routes are replaced, not kept beside the new ones.

### 2. API

- `GET /workspaces/{ws_id}/later?status=&cursor=` — the caller's items with the message they
  point at (author, channel, content, attachments) in one round trip; overdue first in *In
  progress*, then by `created_at`.
- `POST /workspaces/{ws_id}/later` — `{ message_id }` (save for later), or `{ text, remind_at,
  for_user_id? }` (a reminder; for someone else only if they share the workspace, as `/remind`
  allows today). Saving a message twice returns the existing item.
- `PATCH /later/{id}` — `status`, `remind_at` (set, change or clear), `note`. Completing sets
  `completed_at`; setting a new `remind_at` clears `reminded_at`.
- `DELETE /later/{id}`.
- `/remind` creates an item; the reminder checker in `chat-worker` reads `later_items` and, as
  today, sends the in-app notification and push, then stamps `reminded_at`.
- A `later.updated` frame to the owner's sessions so every tab and device shows the same list.

### 3. The panel and the message

- **Sidebar**: *Later* with the count of *In progress* items, replacing *Saved items* and
  *Reminders*.
- **Panel** (`PanelHeader`): tabs *In progress* · *Archived* · *Completed*. Each item shows the
  message preview (author, channel, text, compact attachments) or the reminder text, a due
  chip — red when overdue, and a checkbox that completes it. Its menu: *Remind me* (in 20
  minutes · 1 hour · 3 hours · tomorrow · next week · custom), *Archive*, *Remove*. Clicking it
  jumps to the message.
- **On a message**: *Save for later* in the hover bar (filled when saved), and *Remind me about
  this* with the same presets, which saves it with a due time in one step. The keyboard
  shortcut `A` saves the focused message, as in Slack.
- A fired reminder's notification opens the item.

## Acceptance

- [ ] Saving a message puts it *In progress*; ticking it moves it to *Completed*; archiving to
      *Archived*; each can be moved back.
- [ ] A reminder — on a message or from `/remind` — fires once at its time, stays *In progress*
      until ticked, and can be snoozed from the list.
- [ ] Existing saved messages and reminders appear in *Later* after the upgrade with nothing
      lost; delivered reminders show as fired.
- [ ] The sidebar count and the list stay in step across tabs and devices.
- [ ] A member who leaves the workspace loses their items; nobody sees anyone else's.

## Tests

`http_tests` for the routes (owner-only, cross-workspace refused, save twice is one item,
status transitions, reminder for another member, no-token / non-member), the migration on
seeded saved messages and reminders, and the checker firing once and stamping `reminded_at`.
Frontend tests for the tabs, the checkbox, the due chip's overdue state and the remind presets.
