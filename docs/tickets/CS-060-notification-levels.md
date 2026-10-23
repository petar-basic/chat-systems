# CS-060 — Notification levels, and DMs that reach your phone

**Wave:** 21 — notifications that fit how a team works
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-061, CS-062

## Problem

A channel has two notification states: muted, or not. Slack has three levels per channel
(all new messages, mentions only, nothing) plus a default the user chooses once, and a team
arriving from Slack sets them in the first week: everything in `#incidents`, nothing in
`#random`.

Three things are wrong today, and they are one problem:

- **A direct message does not reach a phone.** The notification consumer
  ([`notifications/consumer.rs`](../../backend/api/src/notifications/consumer.rs)) acts on
  mentions and thread replies. A DM without an `@` is neither, so it creates no notification,
  no push and no email. The browser shows one only while a tab is open, decided in
  [`wsQuerySync.ts`](../../frontend/src/lib/wsQuerySync.ts) from `message.new`.
  `NotificationType::Dm` exists and nothing produces it.
- **Every channel message plays the sound.** The same client code calls `playMessageSound`
  for any incoming message in an unmuted channel. With twenty active channels the only
  escape is muting them, which also hides the unread state.
- **The decision is made twice.** The server decides for mentions and replies, the client
  for DMs and sounds. They disagree, for example on DND: the server honours `dnd_until`, the
  client does not.

`channel_members` still carries two columns from the initial schema that nothing writes:
`notifications VARCHAR(20) DEFAULT 'default'` and `is_muted`. The working mute is the later
`muted` column.

## Approach

### 1. One level per membership, and a default per person

- A `notify_level` enum: `all`, `mentions`, `nothing`.
- `workspace_members.notify_default notify_level NOT NULL DEFAULT 'mentions'`: the
  "Notify me about" choice, per workspace, as in Slack.
- `channel_members.notify_level notify_level NULL`. `NULL` means "use my default". The
  migration drops the dead `notifications` and `is_muted` columns and removes `is_muted`
  from the queries and models that still select it
  ([`workspace/repo.rs`](../../backend/api/src/workspace/repo.rs)).
- **DMs and group DMs** default to `all` whatever the workspace default says. A DM is
  addressed to you; Slack treats it as a mention. The person can still set a DM to `nothing`.
- **Mute stays its own switch.** Muted means no notifications at any level, plus a dimmed
  sidebar row with no bold. A muted channel keeps its stored level and gets it back when
  unmuted.
- `PUT /channels/{id}/notify-level { level: "all" | "mentions" | "nothing" | null }` and
  `PUT /workspaces/{id}/notify-default`. The channel list and conversation payloads return
  the effective level, so the client never resolves the default itself.

### 2. The server decides, the client only shows

- The consumer resolves the effective level for every recipient in one query per message,
  joined to `muted` and DND, not one query per recipient as `is_channel_muted` does today.
- **`all`:** every member except the author gets the `notification` frame and, when
  offline, a push. No row in the Activity panel: Activity is for things addressed to you,
  and a row for every message buries the mentions.
- **`mentions`:** as today (mentions, broadcasts, groups, followed-thread replies).
- **`nothing`:** no frame, no push, no email, even for a mention. The unread badge and the
  mention count still move; that is state, not a notification.
- **DMs** produce `NotificationType::Dm`: an Activity row, a push when offline, and the
  email digest under the same rules as a mention ([`email.rs`](../../backend/api/src/notifications/email.rs)).
- The client stops deciding. `wsQuerySync.ts` keeps updating unread state from
  `message.new`, but sound and desktop notifications come only from the `notification`
  frame ([`NotificationStream.tsx`](../../frontend/src/features/notifications/NotificationStream.tsx)).
  The frame gains a `kind` (`mention`, `dm`, `reply`, `keyword`, `message`) so the client can
  choose sound and title without guessing.

### 3. The controls

- **The channel's menu** (header and sidebar row): a "Notifications" submenu with
  Default (shows what the default is), All new messages, Mentions, Nothing, then Mute.
- **The DM header** gets the same submenu.
- **Preferences → Notifications** gets "Notify me about" as `RadioCards` above the push and
  email panels, per workspace.
- The sidebar row shows a small bell-off icon for `nothing` the way it shows mute today, so
  a silent channel is never a mystery.

## Acceptance

- [ ] A DM to an offline person produces an Activity row, a push and, if they opted in, the
      email digest. It produces none of these while they are in DND.
- [ ] A channel set to `all` notifies on every message; `mentions` only on mentions;
      `nothing` never, while unread and mention counts still move.
- [ ] Changing the workspace default changes every channel left on Default and none that
      were set explicitly.
- [ ] Mute overrides every level and restores the stored level when lifted.
- [ ] No sound or desktop notification comes from `message.new`; the server's
      `notification` frame is the only trigger.
- [ ] The dead `notifications` and `is_muted` columns are gone.

## Tests

`http_tests` for both routes (member ok, non-member 403, no token 401, unknown channel 404,
invalid level 422). Consumer tests in the style of [`http_tests/push.rs`](../../backend/api/src/http_tests/push.rs):
a matrix of level × muted × DND × DM/channel × mention/plain asserting frame, Activity row,
push and email. Frontend: the channel menu sets and shows the level; `NotificationStream`
plays the sound for a `message` frame and `wsQuerySync` no longer does. E2E: set `#random`
to Nothing, post a mention from a second user, and no desktop notification fires while the
badge still appears.
