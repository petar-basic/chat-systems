# CS-065 — Polls

**Wave:** 23 — structured work: polls, lists, workflows
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-067

## Problem

"Lunch Thursday or Friday?" is answered today with emoji reactions and a hand count. In Slack
that is a poll (Simple Poll or Polly, both among its most-installed apps), and the reactions
workaround breaks as soon as there are more than three options or somebody wants to change
their answer.

This is also the first message here with **buttons the server owns**: a vote is a click
that changes state. The way it is built becomes the pattern CS-067's workflow forms
reuse, so it gets built as a primitive, not as a one-off.

## Approach

### 1. The model

- `polls (id, message_id UNIQUE, question, multiple_choice BOOLEAN, anonymous BOOLEAN,
  closes_at NULL, closed_at NULL, created_by)`, `poll_options (id, poll_id, position, label)`
  with 2–10 options, `poll_votes (poll_id, option_id, user_id, created_at)` with a primary key
  on `(option_id, user_id)` and, for single choice, a partial unique index on
  `(poll_id, user_id)`.
- The message is an ordinary message whose `metadata.poll` carries the question, the options
  and the **counts**. It is the same move as link previews and attachments: the message
  renders from its own row, history and search need no join, and a vote is a
  `message.updated` like an unfurl. The voters' names are not in the metadata;
  `GET /polls/{id}/votes` returns them, and refuses for an anonymous poll, to everybody
  including its creator.
- **Anonymous means anonymous in the database export too.** The export writes counts for an
  anonymous poll, not `poll_votes` rows.

### 2. The routes

- `POST /channels/{id}/polls` creates the poll and its message in one transaction, staged
  through the outbox like any message.
- `PUT /polls/{id}/vote { option_ids }` replaces the caller's vote (an empty list withdraws
  it), recomputes the counts in the same transaction, and stages `message.updated`.
- `POST /polls/{id}/close`: the creator or a channel moderator
  (`authz::can_moderate_channel`). A closed poll refuses votes and
  shows its final result.
- `closes_at`: the `reminder_checker` loop in `chat-worker` closes due polls.
- Authorization is `authz::require_channel_post` for creating and
  `require_channel_access` for voting. A guest outside the channel cannot vote by guessing an
  id; an announcement channel lets everybody vote, though only admins post.

### 3. The UI

- **Creating:** `/poll` opens the dialog, and so does a poll button in the composer toolbar
  next to Attach files. `/poll "Question" "A" "B"` with quoted arguments skips the dialog. The dialog is a
  CS-059 `Modal`: question, options with add/remove and drag to reorder, single or multiple
  choice, anonymous, an optional close time.
- **The card:** the question, one row per option with a bar, a count and the caller's own
  choice marked. Voters' avatars appear unless the poll is anonymous. A footer reads "12
  votes · closes Friday" or "Closed". Rows are buttons with `aria-pressed`, reachable by
  keyboard from the message's roving focus (CS-058).
- A vote updates the card optimistically and rolls back on error.

## Acceptance

- [ ] A poll is created from the dialog and from `/poll` with arguments, in channels, DMs
      and threads.
- [ ] Single choice keeps one vote per person under concurrent clicks; multiple choice keeps
      any subset; changing and withdrawing a vote work.
- [ ] Every client sees counts update live; voters' names only on non-anonymous polls.
- [ ] Closed and expired polls refuse votes; only the creator or a
      channel moderator closes one.
- [ ] Nobody can learn who voted what on an anonymous poll, the creator and the export
      included.

## Tests

`http_tests`: the authorization matrix for every route, single-choice under two concurrent
votes, multiple choice, withdraw, close permissions, `closes_at` via the dispatcher, the
anonymous voters route refused, guests. Export test for an anonymous poll. Frontend: the
dialog, `/poll` argument parsing, the card's optimistic vote and rollback, keyboard voting.
E2E: two users vote and each sees the other's vote arrive.
