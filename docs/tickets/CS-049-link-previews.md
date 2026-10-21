# CS-049 — Link previews

**Wave:** 14 — the daily-use gaps a team arriving from Slack notices first
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

A pasted link renders as bare text. In Slack nearly every link a team shares — a PR, a doc,
a ticket, an article — arrives with its title, site and image, and people read the card
instead of opening the link. Its absence is the first thing somebody coming from Slack
remarks on, and nothing in the codebase fetches or stores a preview today.

## Approach

1. **Fetch in the worker, never in the request.** When a message is created or its content
   edited, the worker takes the first three `http(s)` URLs that are not inside a code span,
   via the event it already consumes, and fetches each with `hooks::ssrf::validate_outbound_url`
   — the guard outgoing webhooks and the Slack importer already use. Resolve once and
   connect to the resolved address so a DNS answer cannot change between check and fetch.
   3 s timeout, 512 KB body cap, `text/html` only, no cookies, a `Chat-Systems/x.y`
   user agent.
2. **Parse OpenGraph, then `<title>`.** `og:title`, `og:description`, `og:site_name`,
   `og:image`; fall back to `<title>` and `<meta name=description>`. Truncate fields; keep
   the image as a URL, not a proxied copy — proxying images is its own ticket if wanted.
3. **Store on the message.** `metadata.unfurls: [{url, title, description, site, image}]`,
   written by the worker and announced with the existing `message.updated` frame, so the
   client needs no new event. Cache by URL for 24 h in Redis so a link pasted into ten
   channels is fetched once.
4. **Let people say no.** The author can remove a preview from their message; a workspace
   admin can turn previews off for the workspace (some teams do not want the server
   fetching arbitrary URLs at all).
5. **Render a card** under the message body: site, linked title, two lines of description,
   image thumbnail; nothing while the fetch is pending.

## Acceptance

- [ ] A link to a public page gets a card within a couple of seconds of sending.
- [ ] Private, loopback, link-local and metadata addresses are never fetched, including via
      a redirect or a DNS answer that changes.
- [ ] Editing a message re-unfurls; removing a preview sticks across edits.
- [ ] The workspace setting turns previews off for new messages.
- [ ] A page that times out, is too large or is not HTML produces no card and no error.

## Tests

Worker tests against a local HTTP fixture: OpenGraph parsing, `<title>` fallback, size and
time limits, the redirect-to-private case refused. `http_tests` for the remove-preview and
workspace-setting authorization matrix. One E2E: paste a link to the stack's own frontend,
see the card.
