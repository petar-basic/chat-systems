# CS-136 — An app's Home tab and its shortcuts

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend · docs
**Blocked by:** CS-134 · CS-135
**Blocks:** —

## Problem

An app only lives in channels. It has no page of its own to show a person their open
requests or settings, and starting it means remembering a slash command. Slack apps have a
*Home* tab in their DM, and shortcuts in the composer's menu and on every message's
*More actions*.

## Approach

- **Home:**
  - An app's DM gets two tabs, *Home* and *Messages*. *Home* is a view the app publishes
    for each person through a new route, using the blocks of CS-134 and the interactivity of
    CS-135.
  - Opening *Home* sends `app_home_opened`, new in the events list.
  - An app that has never published shows a short placeholder with its description.
  - Turning *Home* on or off is on the app's page.
- **Shortcuts** declared on the app's page, each with a name, a description and a
  `callback_id`:
  - **Global** shortcuts are listed in the composer's shortcut menu (`ShortcutMenu.tsx`),
    next to the workflow shortcuts, and in a search for *shortcuts*.
  - **Message** shortcuts are listed under *More actions* on every message in a channel
    where the app is a member.
  - Using one sends a `shortcut` or `message_action` payload with a `trigger_id`, the
    channel, and for a message shortcut the message. The app usually answers by opening a
    modal.
- **Who sees what:** a guest sees no app shortcuts. A disabled app's shortcuts and Home tab
  disappear, and its payloads are not sent.
- **`docs/bot-api.md`** gets the Home route, the event and both payloads.

## Acceptance

- [ ] A test app publishes a Home for Ana listing her requests. Bob's Home shows his own,
  and the tab updates when the app publishes again.
- [ ] *New request* from the composer's shortcut menu opens the app's modal.
- [ ] *Create ticket from message* on a message sends that message to the app, which opens
  a modal prefilled with its text.
- [ ] A message shortcut does not appear in a channel the app is not in, and none appear for
  a guest.
- [ ] Disabling the app removes its shortcuts and Home without a reload.

## Tests

`http_tests`:
- publishing Home per person, where nobody can read another person's view;
- `app_home_opened`;
- shortcut payloads and their visibility rules;
- a disabled app.

Frontend: the tabs and both menus. E2E with the test app.
