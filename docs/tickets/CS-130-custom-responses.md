# CS-130 — Custom responses from the workspace's bot

**Wave:** 34 — writing and finding (2026-11-13)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-140

## Problem

Every team answers the same questions in channels: the Wi-Fi password, the VPN guide, where
the holiday calendar is. Slackbot's custom responses let an admin say *when somebody writes
"wifi password", answer with this*. Here somebody pastes it again, every time.

## Approach

- **Built-in bot users:**
  - A workspace gets bot users of its own, made on first use (`users.is_bot`, as apps'
    bots are). They have a kind (`responses` here, the calendar bots of CS-140 later), the
    instance's icon and a name an admin can change.
  - They are not apps and have no token. They show with an *App* badge like other bots.
- **Responses** in *Workspace settings → Custom responses*:
  - Each holds one or more trigger phrases and one or more answers; an answer is picked at
    random.
  - Who may edit is a `WhoMay` rule, `members` by default as in Slack. Every change is in
    the audit log.
- **Matching runs in `chat-worker`**, on the message stream, idempotent on the message id:
  - A phrase matches as a whole phrase, ignoring case. In text without spaces between
    words, such as Japanese, it matches as a substring.
  - The answer goes into the same channel or thread, at most once per message. The same
    phrase answers at most once a minute in one channel.
  - Messages from bots, hooks and workflows never trigger.
  - Where the author could post, the bot posts. That rules out announcement channels for
    anyone but moderators, archived channels, and 1:1 DMs. A 1:1 DM is never answered.

## Acceptance

- [ ] An admin adds *wifi password* → *It's on the fridge*. A member writes *what's the WiFi
  password?* and the workspace's bot answers in the channel within a few seconds.
- [ ] Answering in a thread answers in that thread.
- [ ] Two answers alternate at random over many messages.
- [ ] A bot's message with the phrase gets no answer, and the same phrase twice within a
  minute gets one.
- [ ] A rule of `admins` hides editing from members, and the routes answer `403`.

## Tests

Unit: phrase matching, including Japanese text and punctuation around the phrase.
`http_tests`:
- CRUD and its authorization matrix;
- the worker answering once on a replayed stream entry;
- the cool-down;
- bots and archived channels ignored.
