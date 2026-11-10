# CS-121g — Threads that know where they are

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** frontend · backend
**Blocked by:** CS-121f
**Blocks:** CS-121h

## Problem

Confirmed in the code:

- **Threads from an archived channel, or a public one you left, read "Direct message" and open
  a spinner that is remembered (MSG-17).** `list_followed` returns followed threads in archived
  channels and in public channels you are not in. The client knows only the channels you are in
  that are not archived, and `ThreadsView`'s `placeOf` treats every root whose channel is not in
  that list as a conversation: the card says *Direct message*, its title opens the conversation
  route, which has no fallback for an id that is not a conversation and spins forever, and
  `lastPlace` remembers that place, so switching workspace and back lands on the spinner again.
  The card also offers a composer whose replies get 403. Bulk archive (4.4.0) makes this
  common.
- **A thread whose root is deleted while you look at it keeps its composer.** `ThreadPanel`
  shows *This thread was deleted* through `isClosedThread(parentMessage, …)`, but
  `parentMessage` is the copy put into `useRightPanel` when the panel opened, and
  `message.deleted` does not touch it. With no replies left, every reply then fails.
- ***Also send to channel* stays ticked after scheduling.** `ThreadPanel` clears it in
  `handleSend`; scheduling goes through `MessageInput`'s schedule path and leaves it set, so the
  next ordinary reply goes to the channel too.

## Approach

- `ThreadSummary` carries the root's channel name, type and whether it is archived, and whether
  you are in it. `placeOf` uses that instead of the sidebar's channels: an archived channel's
  card has no composer and its title opens the channel read-only; a public channel you left
  opens the way it opens from *Browse channels*.
- A conversation route whose id is not a conversation you are in sends you to the workspace's
  root, and `lastPlace` forgets it.
- The panel reads its root from the message cache by id, so a deletion reaches it, and keeps the
  copy it opened with only for a root that is not in any loaded page.
- Scheduling a reply clears *Also send to channel*, in the panel and on the Threads card.

## Acceptance

- [ ] Follow a thread, archive its channel, open *Threads*: the card names the channel, has no
  composer, and its title opens the archived channel.
- [ ] A remembered place that is no longer a conversation opens the workspace's root.
- [ ] Deleting a root with no replies while its panel is open shows *This thread was deleted*.
- [ ] After scheduling a reply with *Also send to channel*, the box is clear.

## Tests

`threads` repo test for the summary's channel fields; unit tests for `placeOf`, the route
fallback, the panel after `message.deleted`, and the checkbox after scheduling.
