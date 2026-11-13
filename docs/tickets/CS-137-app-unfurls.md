# CS-137 — Apps that preview their own links

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend · docs
**Blocked by:** CS-134
**Blocks:** —

## Problem

The link previews of CS-053 and CS-054 cover Jira, GitHub, Google Drive, Figma and
Confluence, built in. A company's own tools get the generic page preview, or nothing when
they are behind a login: the internal tracker, the admin panel, the wiki. In Slack, an app
claims a domain and draws the preview itself.

## Approach

- **Domains** listed on the app's page, at most five, with the `links:read` and
  `links:write` scopes.
  - Two apps in a workspace cannot claim the same domain.
  - A built-in provider's domain cannot be claimed while that provider is on.
- **When a message links to a claimed domain:**
  - The unfurl consumer (`messaging/unfurl/consumer.rs`) skips its own fetch for that URL.
  - It sends `link_shared`, with the channel, the message and the matching links, to the app
    if the app may read that channel. Otherwise the URL gets no preview.
- **The answer:**
  - A new route takes blocks per URL, within 30 minutes of the event, for that message
    only.
  - The preview is stored with the message's other unfurls and sent as `message.updated`.
  - It is drawn like a built-in one, with the app's name and icon.
  - The *Remove preview* that exists (`DELETE /messages/{msg_id}/unfurls`) removes it, and
    the app is not asked again for that message.
- **Workspace link previews off** (CS-070) turns these off too.
- **`docs/bot-api.md`** gets the scopes, the event and the route, with an example.

## Acceptance

- [ ] A test app claims `tracker.example.com`. A member posts a link to it, the app gets
  `link_shared`, answers with a header and fields, and both browsers show the preview.
- [ ] The same link in a channel the app cannot read gets no preview, and the app gets no
  event.
- [ ] A domain already claimed, or `github.com` while GitHub previews are on, is refused.
- [ ] Removing the preview keeps it removed when the message is edited.
- [ ] An answer after 30 minutes, or for a message that did not link to the domain, is
  refused.

## Tests

`http_tests`:
- domain claims and collisions;
- the event and its reach;
- the answer route's window and scope checks;
- removal kept across edits;
- link previews off.
