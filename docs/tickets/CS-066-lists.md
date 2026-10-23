# CS-066 — Lists

**Wave:** 23 — structured work: polls, lists, workflows
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-067 part 4

## Problem

A team tracks small structured things beside its conversations: a launch checklist, the bug
intake from support, who is on call which week, a hiring pipeline. Today that is a pinned
message edited by hand, or a link to a spreadsheet somewhere else. Slack Lists is a table
inside the workspace: typed columns, people and dates, a board view by status, and a
comment thread on every item.

Canvases (free-form documents) are a separate decision and not part of this ticket; see
[ROADMAP.md](../ROADMAP.md#what-is-deliberately-not-on-this-list).

## Approach

Four parts, each its own PR, in this order.

### 1. The model, the API and live updates

- **Where a list lives decides who sees it.** A list belongs to one conversation (a
  channel, DM or group DM), and access is that conversation's access through
  `authz::require_channel_access`, with no second permission system. Editing needs
  `require_channel_post`, so an announcement channel's list is read-only for everyone except
  its posters. A list with no conversation is personal: its creator only. Sharing one list
  into several channels is deliberately left out; it is what makes Slack's list permissions
  hard to reason about.
- `lists (id, workspace_id, channel_id NULL, title, description, created_by, created_at,
  updated_at, deleted_at)`.
- `list_columns (id, list_id, position, name, kind, options JSONB)`. The kinds are `text`,
  `number`, `select`, `multi_select`, `person`, `date`, `checkbox` and `link`. `options`
  holds a select's choices (id, label, colour) and a number's format. Limit 30 columns.
- `list_items (id, list_id, rank TEXT, values JSONB, created_by, created_at, updated_at,
  deleted_at)`. `values` is keyed by column id. `rank` is a fractional index, so a move
  writes one row. Limit 10,000 items.
- **Concurrent edits merge per cell.** `PATCH /list-items/{id} { values: { col: value } }`
  writes `values = values || $1`, so two people editing different cells of one row both
  win, and the same cell is last-write-wins. Values are validated against the column's
  kind on the server: a `person` must be a member of the conversation, a `select` id must
  exist.
- Changing a column's kind converts the cells it can (number ↔ text, select ↔ text) and
  clears the rest after a confirmation that names how many will be lost.
- **Live updates** through the outbox to the conversation's audience: `list.item_upserted`,
  `list.item_deleted`, `list.columns_changed`, `list.deleted`, each a `ServerFrame` variant
  carrying the row as written.
- Channel deletion, retention and the export cover lists. The export writes each list as
  CSV next to the channel's messages.

### 2. The table

- A list opens in the main area at `/app/:workspaceId/lists/:listId`, like a conversation.
  The channel header gets a **Lists** entry beside bookmarks, and a "New list" action with
  templates: blank, project tracker, bug intake, on-call rota.
- The table is a keyboard grid: arrows move, Enter edits, Escape cancels, Tab moves right,
  typing starts an edit. Each kind has its editor: a `Select` for select, a member picker for
  person, a date picker. Rows are virtualized; 10,000 items scroll smoothly.
- Add, rename, reorder, hide and delete columns from the header cell's menu. Drag rows to
  reorder. An item opens in a side panel with every field.

### 3. Views: board, filter, sort

- **Board view** groups by any `select` column; dragging a card between columns sets the
  value. **Table view** sorts and filters by any column.
- Views are saved on the list (`list_views (id, list_id, kind, config JSONB, position)`) and
  shared by everyone who can see it, as in Slack. "Only mine" and "Due this week" are
  filters, not separate features.

### 4. Comments, assignment and due dates

- Every item has a comment thread (`list_item_comments`) with mentions, rendered by the
  message renderer. A mention notifies through the CS-060 path with
  `NotificationType::List`.
- Setting a `person` cell notifies that person ("Ana assigned you to *Fix login* in Launch
  plan"). Assigning yourself does not.
- A `date` column can be marked as the due date. An item due tomorrow and assigned to you
  shows up in Later ([CS-057](../history.md#wave-18--what-a-slack-user-notices-in-the-first-hour--shipped)) as a reminder you can complete, and completing
  it checks the item's `checkbox` column if the list has one.

## Acceptance

- [ ] A list in a channel is visible and editable exactly as the channel's messages are;
      a personal list only to its creator; guests only through channels they are in.
- [ ] All eight column kinds edit, validate on the server and survive a reload.
- [ ] Two people editing different cells of one row both keep their edit; every viewer sees
      edits live.
- [ ] The table is fully usable from the keyboard and scrolls 10,000 items smoothly.
- [ ] Board view, saved views, sorting and filtering work and are shared.
- [ ] Item comments, assignment notifications and due-date reminders in Later work.
- [ ] Exports, retention and channel deletion include lists.

## Tests

`http_tests`: the authorization matrix for every route (channel member, non-member, guest
outside, announcement channel non-poster, personal list), value validation per kind, the
per-cell merge under two concurrent PATCHes, rank moves, kind conversion, limits. Frame tests
for each `list.*` event's audience. Frontend: grid keyboard navigation, every cell editor,
board drag, the side panel. E2E: two users edit the same list and see each other's edits.
