# CS-083 — Notifications that arrive once, where they should

**Wave:** 26
**Area:** backend · frontend · realtime
**Blocked by:** —
**Blocks:** —

## Problem

- *Email me what I missed* goes out when a push was accepted by the push service but never
  shown, and is skipped when it should go; the link in it names no workspace.
- Push is suppressed whenever any socket is open, even one in a tab nobody has looked at for
  hours.
- Pausing notifications does not silence the huddle ring in the gateway; the pause menu does
  not name the workspace it pauses.
- A webhook's creator is never notified of what their own hook posts, and a bot's push shows
  the hook's creator as the author instead of the bot.
- The in-tab desktop notifications setting exists but has no switch.

## Approach

- The digest decides at send time from what is still unread after the window, whatever
  happened to push, and links to the workspace and the message.
- The client reports active/idle (visibility and input, with a timeout) over the socket;
  push is suppressed only while a session is active. Measure first how long mobile browsers
  keep a background socket, and write the numbers in the ticket's PR.
- The ring respects `notifications_paused_until` for the workspace (CS-076); the pause menu
  says *Pause notifications in {workspace}*.
- A bot message notifies its hook's creator like anyone else, titled with the bot's name.
- *Preferences → Notifications* gets the desktop-notifications switch.

## Acceptance

- [ ] An unseen mention emails once; a seen one never does.
- [ ] An idle open tab does not swallow push.
- [ ] A paused workspace does not ring.

## Tests

Digest with read and unread mentions, idle vs active sessions on the push path, ring under
pause, bot push title.
