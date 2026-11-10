# CS-121h — Slash commands in every message box

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** frontend · backend
**Blocked by:** CS-121a, CS-121c, CS-121g
**Blocks:** CS-121l

## Problem

Confirmed in the code:

- **A command typed in a DM or a thread is sent as a message (MSG-18).** Only the channel
  composer parses commands (`useComposer`); `ConversationView` and `ThreadPanel` send the raw
  text, and their `MessageInput` has `slashCommands` off, so there is no autocomplete either.
  Ana types `/dnd 60` in her DM with her manager and it goes to him as a message. The backend
  already accepts any channel, DMs included.
- **An answer that never comes waits forever, and a second command overwrites the first.**
  *"/x is taking a moment…"* is a `pending` answer with no timeout (only settled answers
  dismiss after 10 s), and `command.response` is a live frame, so a dropped socket loses it.
  `useCommandAnswerStore` holds one answer for the whole app, so a second command's answer
  replaces the first one's.

## Approach

- The command branch of `useComposer` becomes one hook the three composers share, with
  autocomplete on in each. In a DM it runs as in a channel; CS-121a already refuses what does
  not belong there (`/invite`, `/topic`) and posting into a closed DM.
- From a thread, `invoke` takes `thread_parent_id`: an `in_channel` answer is posted as a reply
  in that thread, and a registered command receives the thread id in its payload, as Slack's
  `thread_ts`.
- Answers are kept per place (channel, DM or thread). A pending answer ends after the response
  URL's lifetime with "No answer from /x", and a reconnect does not leave it spinning.

## Acceptance

- [ ] `/away` in a DM sets you away and sends nothing; `/shrug` in a thread posts in the thread.
- [ ] Autocomplete offers commands in a DM and in a thread.
- [ ] A command whose app never answers stops waiting with "No answer from /x".
- [ ] Two commands in two channels each keep their own answer.

## Tests

`http_tests`: `invoke` with `thread_parent_id` for a built-in and a registered command. Unit
tests for the shared hook in each composer and for the answer store. E2E: `/away` in a DM.
