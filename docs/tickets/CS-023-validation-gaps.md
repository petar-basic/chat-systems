# CS-023 — Close remaining input validation gaps

**Wave:** 5 — Correctness
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit findings:** S15, S16 (LOW)

## Problem

`shared_common::validation` covers email, password, display name, workspace name, channel
name, message content and avatar URL. Three inputs reach the database without passing
through it.

**Reaction emoji.** `add_reaction` in both
[conversations](../../backend/api/src/conversations/routes.rs#L311) and
[messaging](../../backend/api/src/messaging/routes.rs#L369) binds `req.emoji` straight
into a `VARCHAR(50)` column. A longer value produces a Postgres error surfaced as
`AppError::Database` — a 500 where the correct answer is 400. A 50-character "emoji" also
renders in every client's reaction bar. The realtime huddle path already gets this right
(`emoji.chars().count() > 8`,
[`ws_handler.rs:421`](../../backend/realtime/src/ws_handler.rs#L421)) — the HTTP path
does not.

**Reminder content.** [`create_reminder`](../../backend/api/src/hooks/routes.rs#L250)
inserts `req.content` into a `TEXT` column with no length check at all, while every other
user-authored text in the product is capped at 4000.

**Channel topic and description.** `update_channel` writes `topic` (`VARCHAR(500)`) and
`description` (`TEXT`) with no validation, same class of problem.

None of these is a security issue. They are the difference between a product that returns
400 with a message and one that returns 500 and logs a database error — and a 500 on a
user-supplied value is a monitoring false positive that trains people to ignore alerts.

## Approach

1. **Add the missing validators** to `shared/common/src/validation.rs`, following the
   existing shape (trim, empty check, length check, `AppError::Validation`):
   ```rust
   pub fn validate_reaction_emoji(emoji: &str) -> AppResult<()>
   pub fn validate_reminder_content(content: &str) -> AppResult<()>
   pub fn validate_channel_topic(topic: &str) -> AppResult<()>
   pub fn validate_channel_description(description: &str) -> AppResult<()>
   ```
   - emoji: non-empty, at most 8 `chars()` — matching the realtime rule so the two paths
     agree — and rejecting control characters.
   - reminder content: reuse the 4000-character message limit.
   - topic: 500 to match the column; description: 4000.
2. **Call them at the top of each handler**, before any repo call, consistent with how
   `validate_message_content` is used today.
3. **Audit the rest in the same pass.** Walk every `Json<T>` request DTO and confirm each
   `String` field either has a validator or is a constrained enum. Record the result in
   this ticket so the next reviewer does not have to repeat the walk. Candidates to check:
   workspace `description` and `icon_url`, user `bio` and `timezone`, hook `name` and
   `description`, invite `email` (validated on the provisioning path but not on
   `create_invite` itself).
4. **Convert unique-violation errors to 409 rather than 500** wherever they can be
   triggered by user input. `is_unique_violation` already exists in
   [`conversations/routes.rs`](../../backend/api/src/conversations/routes.rs#L400) — move
   it to `shared_common::errors` as a helper on `AppError` so every feature uses it
   instead of surfacing a raw database string.

## Acceptance

- [ ] Over-long emoji, reminder content, topic and description return 400 with a clear
      message, never 500.
- [ ] HTTP and WebSocket agree on the emoji rule.
- [ ] Every `String` field in every request DTO is accounted for; the survey result is
      recorded in this file.
- [ ] Unique violations surface as 409 instead of `AppError::Database`.

## Tests

Unit tests in `validation.rs` for each new validator, at and over the boundary, following
the existing table of cases. `http_tests`: one over-limit request per affected endpoint
asserting 400. A test that a duplicate reaction returns 409, not 500.
