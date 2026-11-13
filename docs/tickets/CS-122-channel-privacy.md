# CS-122 — A channel that becomes private, or public

**Wave:** 33 — channels and the people in them (2026-11-13)
**Area:** backend · frontend · realtime
**Blocked by:** —
**Blocks:** CS-123

## Problem

A channel's type is fixed when it is made. A team that opened `#hiring` as public and now
discusses candidates in it has two ways out: archive it and start a private one, which splits
the history and its links, or keep talking where every member can read. Slack lets an admin
turn a public channel private, and a private one public.

## Approach

- **Who:** workspace admins, in *Channel details → Settings*. A DM, a group DM, an archived
  channel and a channel new members join by default (`is_default`) cannot change type.
- **Public → private:**
  - The channel's members stay. Everybody else loses it at once: the channel, its messages,
    files, pins, bookmarks and search hits. That is already what `require_channel_access`,
    `searchable_channels()` and the file routes answer for a private channel, read from
    `channel_type` on every call. The ticket adds tests that hold this, not new checks.
  - The realtime gateway drops the channel for every connected non-member, the way a removal
    does since CS-007. The channel leaves their sidebar and *Browse channels*.
  - A bot with `chat:write.public` that is not a member stops posting there. An incoming hook,
    a workflow or a reminder whose owner is not a member stops, through the `still_acting`
    checks of CS-121c.
- **Private → public:** every member of the workspace can now read the whole history, so the
  dialog says so, names how many messages and files that is, and asks again. Guests still see
  only the channels they are in.
- **A system message** in the channel (`kind: "channel_privacy_changed"`) and an audit entry
  `channel.privacy_changed` with the admin, the old type and the new.
- **Links:** a link to a message in a channel that went private keeps working for its members.
  Anybody else gets what a link into a private channel they are not in gets today.
- **Slack import:** a re-run keeps the type an admin chose here, the way CS-121d keeps a rename.

## Acceptance

- [ ] An admin turns a public channel private. A non-member's open tab loses it without a
  reload, and search, files and message links stop answering for them.
- [ ] An admin turns a private channel public after a second confirmation. Members see it in
  *Browse channels* and can read the history; guests who are not in it cannot.
- [ ] A member who is not an admin sees no control, and the route answers `403`.
- [ ] A DM, a group DM, an archived channel and a default channel cannot change type.
- [ ] A bot with `chat:write.public` and a hook whose owner is not a member stop posting into
  a channel that went private.

## Tests

`http_tests`:
- the authorization matrix for the type change;
- every read path for a non-member before and after: messages, thread, files, pins, search,
  file search, message location;
- the bot and hook cases;
- a Slack re-run after a change.

Realtime: a non-member's socket stops getting the channel's frames. E2E: an admin makes a
channel private while a second member watches it disappear.
