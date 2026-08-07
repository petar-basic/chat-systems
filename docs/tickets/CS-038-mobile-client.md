# CS-038 — Mobile client

**Wave:** 9 — Product parity
**Area:** frontend
**Blocked by:** CS-035 (push is the whole point), CS-024, CS-025
**Blocks:** —

## Problem

There is no mobile client. The README is explicit that the UI is desktop-first and that
mobile layout is not a goal.

For an internal chat tool this is the single biggest adoption risk in the list. People
carry phones, not laptops, and a tool that cannot reach them outside the office gets
replaced in practice by WhatsApp groups for anything urgent — at which point the
self-hosted, auditable system no longer holds the conversations that matter, which defeats
the reason for running it.

## Approach

Two viable paths. Decide before writing code; record the decision in this file.

### Option A — Responsive PWA (recommended first step)

Make the existing SPA usable on a phone and rely on the installable PWA plus Web Push
(CS-035).

- Responsive layouts for the sidebar (drawer), message list, composer and huddle window.
- Touch targets, virtual-keyboard-aware scroll anchoring, safe-area insets.
- Reuse everything: one codebase, one API surface, one release.
- Ships in weeks, not months.
- Limits: iOS PWA push requires the user to add to home screen and remains less reliable
  than native; no background VoIP, so huddle calls cannot ring a locked phone.

### Option B — React Native

A native app sharing the API client, types and query hooks.

- Real push on both platforms, real background behaviour, CallKit/ConnectionService for
  huddles.
- Cost: app store accounts, review cycles, signing, a separate release train, and a second
  UI to keep in step with the web app.

**Recommendation: A now, B only if push reliability or call ringing proves to be the actual
blocker.** Option A also derisks B — the responsive work and the API gaps it exposes are
prerequisites either way.

## Approach for Option A

1. **Audit the layout inventory first.** List every screen and panel, and decide per screen
   whether it collapses, becomes a drawer, or is desktop-only. Write the list here before
   touching CSS; a mobile pass without a target inventory becomes an unbounded refactor.
2. **Navigation.** The workspace / channel / thread three-pane becomes a stack on small
   screens, driven by the existing router rather than by local state, so back gestures and
   deep links work.
3. **Composer.** The TipTap editor on mobile needs deliberate handling of the virtual
   keyboard, autocorrect and the mention/emoji suggestion popovers, which are positioned
   for a mouse today.
4. **Message list.** CS-025's virtualization must hold up with a keyboard opening and
   closing — the scroll-anchor tests from that ticket are the ones to extend.
5. **Huddles on mobile** are a separate decision. Audio-only participation is achievable;
   video mesh on a phone is not. Gate by capability and say so in the UI.
6. **Test on real devices.** Add a mobile viewport project to the Playwright config for
   layout regressions, and keep a manual checklist in `docs/manual-qa.md` for the things
   emulation does not catch — keyboard, safe areas, background/foreground transitions.

## Acceptance

- [ ] Decision between A and B recorded here with a date.
- [ ] Every screen in the inventory has a defined small-screen behaviour.
- [ ] Reading, sending, threads, DMs, reactions, mentions and notifications work on a
      phone.
- [ ] Installed PWA receives push (CS-035) and badges correctly.
- [ ] Mobile viewport project runs in CI.
- [ ] Huddle behaviour on mobile is defined and communicated in the UI.

## Tests

Playwright mobile viewport project covering the core journeys. Manual device checklist for
keyboard, safe areas and install flow on iOS and Android.
