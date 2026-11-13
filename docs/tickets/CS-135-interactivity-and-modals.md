# CS-135 — Buttons, menus and modals that reach the app

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend · realtime · docs
**Blocked by:** CS-134
**Blocks:** CS-136

## Problem

After CS-134 an app can show a button but cannot learn that it was pressed. Approving a
deploy, picking an on-call person or filling in a request form all need the click to reach
the app and the app to answer, often with a form in a modal. Slack calls this interactivity.

## Approach

- **Interaction payloads**, in Slack's shape:
  - `block_actions` when an element in a message is used;
  - `view_submission` and `view_closed` for modals.
  - Each carries the person, the channel, the message, the action's `action_id`, `block_id`
    and value, a `response_url`, and a `trigger_id`.
  - They go to the app's request URL, signed as events are, or over its socket in socket
    mode.
  - Unlike events they are not retried: the person is waiting. An app that does not answer
    in three seconds gets a visible *The app did not respond* on the element.
- **Answering:**
  - The HTTP answer body or the socket acknowledgement may carry `response_action`. For a
    submission that is `errors` with messages per `block_id`, `update`, `push` or `clear`.
  - `response_url` replaces, updates or deletes the original message, or sends an ephemeral
    answer to the clicker. It works up to five times within 30 minutes, as in Slack.
- **Modals:**
  - Routes open, update and push a view, with a `trigger_id` that is valid for three seconds
    and used once.
  - A view has a title, blocks, `input` blocks with the elements of CS-134 plus
    `plain_text_input`, `checkboxes` and `radio_buttons`, and submit and close labels.
  - At most three views are stacked.
  - The frontend draws them with the shared `Modal`, as CS-059 requires, and keeps `state`
    values between pushes.
- **Slash commands** get a `trigger_id`, so a command can open a modal.
- **The app's page** in *Apps* gets *Interactivity: on/off*. With it off, elements stay
  disabled as in CS-134.
- **Scopes:** none new. An interaction only reaches the app that posted the message.
- **Audit:** `bot_app.interaction` for each payload sent.
- **`docs/bot-api.md`** documents the payloads, the answers and the view routes, with an
  example app that approves a deploy.

## Acceptance

- [ ] A bot posts *Deploy v2?* with *Approve* and *Reject*. Pressing *Approve* reaches the
  app, which replaces the message with *Approved by Ana* through `response_url`.
- [ ] `/request` opens the app's modal. A submission with an empty required field shows the
  app's error under that field; a valid one closes the modal.
- [ ] A second view pushed on top keeps the first one's values when going back.
- [ ] An app that does not answer shows *The app did not respond*, and nothing changes.
- [ ] A used or expired `trigger_id` is refused.
- [ ] Both work over HTTP and over a socket.

## Tests

`http_tests`:
- payload signing;
- `response_url` limits and reuse;
- `trigger_id` single use and expiry;
- `response_action` errors, update, push and clear;
- interactivity off;
- socket delivery.

Frontend: modal rendering and the state across pushes. E2E with a test app that answers.
