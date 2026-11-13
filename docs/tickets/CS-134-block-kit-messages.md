# CS-134 — Messages built from blocks

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend · docs
**Blocked by:** —
**Blocks:** CS-135 · CS-136 · CS-137 · CS-140

## Problem

A bot, an incoming hook or a slash command answer posts text and files, nothing else. The
tools teams point at Slack send layout instead of text:
- CI posts a build with its status, branch and a *View run* button;
- an alerting tool sends fields and a colour bar;
- a release bot sends a header and a list.

Here `IncomingWebhookPayload` requires `text` and drops `blocks` and `attachments`, so a hook
copied from Slack that sends only blocks fails with `422`, and one that sends both loses the
layout.

## Approach

- **The blocks, in Slack's JSON shape** so existing payloads work unchanged:
  - `header`, `section` (text, `fields`, an accessory), `divider`, `image`, `context`,
    `actions`;
  - elements: `button`, including one with a `url`, `static_select`, `users_select`,
    `conversations_select`, `overflow`, `datepicker`, `image`;
  - text objects `plain_text` and `mrkdwn`, with Slack's `<@U…>`, `<#C…>`, `<url|label>`
    and `*bold*` forms turned into the app's own mentions, links and markup.
  - The legacy `attachments` too: colour bar, `pretext`, `title`, `fields`, `footer`. CI
    tools still send them more than blocks.
- **Validation:**
  - Slack's limits are checked on the server: 50 blocks a message, the text lengths, and
    known types only.
  - A payload outside them is refused with the path of what is wrong, such as
    `blocks[3].text: too long`.
  - Image URLs go through the unfurl fetcher's rules, so a block cannot make browsers fetch
    from a private address.
- **Storage:**
  - Blocks live in the message's `metadata.blocks`.
  - `text` stays and is required unless blocks are sent; without it, a flattened text is
    built from the blocks.
  - That text is what search, notifications, email and the edit history read.
- **Who may send them:**
  - bots, with `chat:write`, on post and edit;
  - incoming hooks;
  - slash command answers, inline and on `response_url`.
  - People's own messages stay markdown.
- **Rendering** in the static renderer of CS-024, inside the message.
  - A `url` button opens its link.
  - Every other interactive element shows but is disabled until CS-135, with a tooltip
    saying the app has not set up interactivity.
- **Slack import:** bot messages that carry blocks or attachments keep them instead of the
  flattened text.
- **`docs/bot-api.md`** lists the blocks supported and the conversions.

## Acceptance

- [ ] A GitHub Actions Slack notification payload, posted unchanged to an incoming hook,
  shows its header, fields and *View run* button. The button opens the run.
- [ ] A payload with only `attachments` shows the colour bar and fields, and the
  notification reads the flattened text.
- [ ] An unknown block type and a 51st block are refused with the path.
- [ ] Search finds a word that is only in a block's text.
- [ ] A Slack import of a bot message with blocks shows the blocks.

## Tests

Unit: `mrkdwn` conversion and flattening. `http_tests`:
- hooks, bots and `response_url` with blocks, attachments and both;
- the limits;
- private-address images refused;
- search on flattened text.

Frontend: the renderer for each block and element, with snapshot tests. A fixture set of
real payloads from GitHub Actions, Grafana, Sentry and Jenkins.
