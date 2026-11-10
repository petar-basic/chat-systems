# CS-121l — The third survey's small things

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** realtime · frontend
**Blocked by:** CS-121h
**Blocks:** —

## Problem

Smaller findings, confirmed in the code:

- **An active reader turns inactive for a beat (NOT-8).** `renew_activity` keeps a connection
  active only while `last_pong.elapsed() <= HEARTBEAT_INTERVAL`, and the ping goes out on the
  same 30-second tick. A pong that arrives a little earlier in one cycle than in the next, which
  is ordinary jitter on a fast network, leaves the connection inactive for one cycle, and someone
  reading without typing gets a push on their phone for what they are looking at. The comparison
  is confirmed; how often it happens is worked out, not measured. A side effect of the NOT-6 fix.
- **An emoticon at the end of a message stays text.** The input rule (`EMOTICON_RULE`) needs a
  space after the emoticon, so `thanks :)` and Enter sends `:)`. A side effect of the 4.4.2
  change that converts after a space.
- **The new-version banner is only on the workspace page.** `NewVersionBanner` is rendered by
  `WorkspacePage` alone, so the instance admin page and the other signed-in pages never offer
  the reload.
- **Drafts are shared by everyone who signs in on a browser.** `stores/drafts.ts` keys a draft
  by place only, while the unsent store is filtered by user, so on a shared computer the next
  person sees the last one's half-written message.

## Approach

- Activity is renewed when the last pong is within the interval plus a margin of five seconds.
- An emoticon at the end of the text is converted when the message is sent, by the same table.
- The banner moves to the signed-in app's layout.
- Drafts are keyed by user as well as by place.

## Acceptance

- [ ] A pong at 30.2 s keeps the connection active; one at 36 s does not.
- [ ] `thanks :)` and Enter sends `thanks 🙂`.
- [ ] The banner appears on the instance admin page.
- [ ] A second account on the same browser sees none of the first one's drafts.

## Tests

One per item, at the layer it is fixed in.
