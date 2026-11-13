# CS-127 — Drafts that follow you, and *Drafts & sent*

**Wave:** 34 — writing and finding (2026-11-13)
**Area:** backend · frontend · realtime
**Blocked by:** —
**Blocks:** —

## Problem

A half-written message lives in `localStorage` (`stores/drafts.ts`) of the browser it was
typed in. Start a reply on the laptop and it is not on the desktop app, and nothing lists the
drafts left around the workspace. Slack keeps drafts on the server and shows them in *Drafts
& sent*, next to scheduled messages and what you sent.

## Approach

- **A `drafts` table**:
  - `(user_id, channel_id, thread_parent_id)` → `content`, `updated_at`, unique per place.
  - `PUT` and `DELETE` routes under `/workspaces/{ws_id}/drafts`, plus a list route.
  - Only the author ever reads a draft. Export includes drafts and erase removes them.
- **The composer** writes through: local first, as today, then the server after a pause in
  typing.
  - The newer `updated_at` wins when a place has both, so a draft typed offline is sent up
    on reconnect.
  - Sending a message deletes its draft.
  - A `draft.updated` frame goes to the author's other sessions only, so an open composer on
    another device takes the text when it is not focused there.
- **Attachments** stay out of drafts. A draft with files keeps its text and says the files
  were left behind.
- ***Drafts & sent*** replaces *Scheduled* in the *You* menu, with three tabs:
  - **Drafts:** each place with a preview and when it was written. A click opens the
    composer there with the draft in it; a row can be deleted.
  - **Scheduled:** the `ScheduledMessagesPanel` that exists, moved here.
  - **Sent:** the messages you sent across channels, DMs and threads, newest first, paged,
    each opening where it was sent.
- **The sidebar** marks a channel holding a draft of yours with a pencil, as Slack does.

## Acceptance

- [ ] A draft typed in the browser shows in the desktop app's composer for the same channel,
  and in *Drafts*.
- [ ] Sending it removes it from *Drafts* on both.
- [ ] A draft typed offline reaches the server after reconnecting, and does not overwrite a
  newer one from another device.
- [ ] *Sent* lists a thread reply and a DM message with links that land on them.
- [ ] Another member can never read your drafts; the routes answer `404` for a place you
  cannot reach.

## Tests

`http_tests`:
- the draft routes and their authorization matrix;
- newer-wins;
- delete on send;
- *Sent* paging, with no deleted messages and no messages of channels you have left;
- export and erase.

Frontend: the sync rules in the store. E2E: a draft in two browser contexts.
