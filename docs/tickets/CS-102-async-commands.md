# CS-102 — Slash commands that may take their time

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-103

## Problem

A registered slash command is one HTTP request made inside the person's own request
([`commands/routes.rs`](../../backend/api/src/commands/routes.rs), `dispatch`): the app has
three seconds to do its work and answer, there is no retry when the first attempt fails to
connect, and there is no way to answer later. Anything slower than three seconds — a deploy,
a search in another system, a report — cannot be a command, and a brief network blip loses
the command with nothing but an error.

## Approach

- **Acknowledge, then answer.** The app still gets the request at once and may answer in the
  body as today. It may also answer with an empty `200` within three seconds and post its
  answer later to the `response_url` the request carries.
- **`response_url`**: valid for 30 minutes and five uses; each takes `{ text, response_type,
  replace_original? }`, `ephemeral` answers reaching only the person who typed the command
  over their socket. Signed and single-purpose, so it is useless for anything but this
  command's answers.
- **Retry the delivery, not the work.** A command whose request could not be delivered
  (connection refused, timeout before any response, 502–504) is retried twice with backoff
  from the worker, with the same `X-Chat-Request-Id`, so an app can drop a repeat. The
  person sees *`/deploy` is taking a moment…* and then the answer or a final error. A 4xx
  or a 500 is not retried.
- The executor reuses the outgoing-hook signing and SSRF checks.

## Acceptance

- [ ] A command can answer up to 30 minutes later, in channel or only to the caller.
- [ ] A command whose app is briefly unreachable is delivered on a retry, at most once to an
  app that dedupes on the request id.
- [ ] A command that answers in time behaves exactly as before.

## Tests

API: delayed answer through `response_url`, its expiry and use limit, ephemeral answers
reaching only the caller, retry on connection failure and 503 but not 400 or 500, the same
request id on every attempt. Frontend: the pending line and its replacement.
