# CS-114 — Enter to send or for a new line, and where a channel opens

**Wave:** 30 — make it yours (2026-11-04)
**Area:** frontend
**Blocked by:** CS-113
**Blocks:** —

## Problem

Enter always sends and Shift+Enter always makes a new line. People who write long messages
and code ask for the reverse, and Slack has it under *Advanced*. A channel with unread
messages always opens where you left it; Slack lets you choose between that and the newest
message, under *Mark as read*.

## Approach

- *Advanced → When writing a message, Enter*: *sends it* (default) or *starts a new line*,
  in which case ⌘Enter or Ctrl+Enter sends. The composer's send button shows the shortcut
  that sends. The setting is a CS-113 preference.
- *Mark as read → When I open a channel*: *start where I left off* (today's behaviour,
  default) or *start at the newest message and mark everything read*.
- The *Keyboard shortcuts* dialog shows the sending shortcut the person chose.

## Acceptance

- [ ] With *starts a new line*, Enter makes a new line in the composer, the thread composer
  and the edit box, and ⌘Enter or Ctrl+Enter sends.
- [ ] Lists, code blocks and the mention and emoji pickers keep their own Enter behaviour in
  both modes.
- [ ] With *start at the newest message*, opening a channel shows its latest message and
  clears its unread count.

## Tests

Frontend: the composer's key handling in both modes, pickers and lists included; the
channel's opening position for each choice. E2E: switch Enter to new line, write two lines,
send with ⌘Enter.
