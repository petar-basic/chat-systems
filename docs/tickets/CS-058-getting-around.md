# CS-058 — Getting around like in Slack: keyboard, search filters, Threads

**Wave:** 19 — getting around
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

A Slack user moves through Slack without the mouse, narrows a search in the box, and keeps up
with conversations they are in from one *Threads* view. Here:

- **Keyboard.** Two shortcuts exist: ⌘K (the switcher) and ⇧⌥A (all unreads). There is no
  way to move between channels, to reach a message without the mouse, to edit your last
  message with ↑, or to see which shortcuts exist. *Later*'s `A` waits on message focus.
- **Search.** The API filters by channel and author, but the panel offers no filter at all,
  and nothing narrows by date or by "has a file / a link".
- **Threads.** A reply in a thread you started or joined is visible only by opening that
  thread. There is no list of your threads, no unread count for them, and the `reply`
  notification type exists but is never sent.

## Approach

Three parts, each its own PR, in this order.

### 1. Keyboard

- **Channels**: ⌥↑ / ⌥↓ previous / next in the sidebar's order; ⌥⇧↑ / ⌥⇧↓ previous / next
  *unread*; Esc marks the open channel read; ⇧Esc marks everything read.
- **Composer**: ↑ in an empty composer edits your last message in the conversation.
- **Messages**: the list is reachable (Tab from the header, ⇧Tab from the composer, or a
  click), ↑ / ↓ move between messages with a visible focus ring and the row's actions shown,
  and on the focused message `E` edits (own), `T` opens the thread, `R` opens the reaction
  picker, `A` saves for later, `P` pins, Enter opens the thread, Esc returns to the composer.
  Roving `tabIndex`, so the list is one Tab stop.
- **⌘/** opens a *Keyboard shortcuts* dialog listing all of the above, grouped, with the
  platform's modifier names.
- Shortcuts never fire while typing in an input, and none collide with the browser's.

### 2. Search filters

- **API**: `GET /search` gains `has` (`file`, `link`, `image`), `before` and `after` (dates),
  next to the existing channel and author filters; `in` accepts a DM too.
- **Panel**: chips under the box — *From*, *In*, *Has*, *Date* — each a small picker, and the
  same filters typed into the box (`from:@ana in:#design has:file before:2026-10-01
  after:…`) become chips as they are recognised, as in Slack. Results say which filters
  apply and *Clear filters* drops them.

### 3. Threads

- **Following.** `thread_follows (user_id, root_message_id, workspace_id, last_read_at,
  following)`. You follow a thread when you start it and somebody replies, when you reply, or
  when you are mentioned in it; *Follow* / *Unfollow* on a thread changes it. A migration
  follows every existing thread for its root author and repliers, read up to now.
- **Unread**: replies after `last_read_at` from someone else. Opening a thread or replying
  marks it read; a `thread.read_state` frame keeps every tab in step.
- **Notifications**: a reply in a followed thread sends the `reply` notification (in-app and
  push, respecting mute and DND), not just mentions.
- **API**: `GET /workspaces/{ws}/threads?cursor=` — followed threads by latest reply, each with
  its root, the last three replies, reply and unread counts; `POST /threads/{root}/read`;
  `PUT` / `DELETE /threads/{root}/follow`.
- **Client**: *Threads* in the sidebar under *All unreads* with the number of threads that
  have unread replies; a view listing them, each with its root, the latest replies, *N new
  replies*, and a reply box in place; *Follow* / *Unfollow* in the thread panel.

## Acceptance

- [ ] ⌥↑/↓ and ⌥⇧↑/↓ move through channels and unread channels; Esc / ⇧Esc mark read; ↑ in an
      empty composer edits the last message; ⌘/ lists every shortcut.
- [ ] Messages can be reached and walked with the keyboard, and `E`, `T`, `R`, `A`, `P`, Enter
      and Esc act on the focused one; nothing fires while typing.
- [ ] Search narrows by author, place, *has* and date, from chips or typed modifiers.
- [ ] A reply in a thread I am in shows in *Threads* as unread, counts on the sidebar entry,
      notifies me, and clears when I read or reply; *Unfollow* stops all of it.
- [ ] Existing threads are followed after the upgrade without a flood of unread.

## Tests

Frontend tests for the shortcut handler (each binding, nothing while typing), message focus
movement and actions, the modifier parser and chips. `http_tests` for the search filters
(each alone and combined, guest scoping unchanged), thread following (auto-follow on start,
reply and mention; unfollow; read marks; unread counts; access when the channel is lost),
the reply notification honouring mute, and the migration's backfill. E2E: walk to a message
and save it with `A`; reply from the Threads view.
