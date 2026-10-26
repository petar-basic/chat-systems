# CS-074 — A channel shows what posts into it, and its admin can add a webhook

**Wave:** 24 — ready for a company
**Area:** backend · frontend
**Blocked by:** CS-073
**Blocks:** —

## Problem

Every integration lives in the workspace admin's panel, and nothing in a channel says
which of them touch it:

- an incoming webhook is bound to a channel through `config.channel_id`, yet the channel
  does not know it exists; a message that arrives from it looks like it came from nowhere;
- outgoing webhooks and slash commands carry `config.channel_ids`, visible only to a
  workspace admin;
- a workflow whose trigger listens to a channel, or whose step posts into one, shows up only
  in *Workflows*.

Creating any hook needs a workspace Admin (`create_hook`), so the team that owns
`#deploys` asks an admin each time it wants a CI notification. Slack puts this in the
channel: an app is added to a channel from its details, and an incoming webhook belongs to
one channel and one person.

Apps (GitHub, Jira, Figma, Google, Confluence) are not part of this: they unfurl links for
the person who connected them and post nowhere, so they stay workspace-wide.

## Approach

### Backend

- `GET /channels/{channel_id}/integrations`, for anyone with access to the channel
  (`require_channel_access`), returns what touches it, without URLs or secrets:
  - incoming webhooks whose `channel_id` is this channel (name, description, who made it,
    when);
  - outgoing webhooks and slash commands whose `channel_ids` include it (name or command);
  - workflows whose trigger's channel is this one or whose `send_message` step posts here
    (name, on or off, who built it).
  One query per kind, filtered in SQL on the `config` and `definition` JSON, not by loading
  the workspace's hooks.
- A channel admin of the bound channel may create, reveal, rotate and delete an
  **incoming** webhook for it. `create_hook` keeps the Admin requirement for every other
  kind, since outgoing webhooks and slash commands send what people type to a third-party
  URL. The check lives in `authz` (`can_manage_channel_hook`) beside
  `require_channel_moderator`, used by create, reveal, rotate and delete alike.
- Each of those writes the audit row it writes today, so a workspace admin still sees every
  webhook made in their workspace.

### Frontend

- *Channel settings* becomes *Channel details*, a `Modal` with tabs: *About* (name, topic,
  description, archive, as today), *Settings* (posting, from CS-075) and *Integrations*.
- *Integrations* lists the three groups above. A channel admin gets *Add an incoming
  webhook*, which shows its URL once, and can rotate or remove one; a workflow row opens it
  in the editor if the viewer may build workflows.
- The workspace *Integrations* section (CS-070) keeps the full list and links each hook's
  channel.

## Acceptance

- [ ] Anyone in a channel can see which webhooks, commands and workflows touch it.
- [ ] A channel admin adds an incoming webhook for their channel without a workspace admin.
- [ ] Outgoing webhooks and slash commands still need a workspace admin.
- [ ] No URL, token or secret leaves the API through the channel route.

## Tests

Backend: the integrations route for a public channel, a private one the caller is not in
(404) and one they are; secrets absent from the response; a channel admin creates, rotates
and deletes an incoming webhook for their channel and is refused for another channel and
for an outgoing webhook; a member who is not a channel admin is refused; the audit rows.
Frontend: the tabs, the integrations list per role, add-webhook shows the URL once.
E2E: a channel admin adds an incoming webhook from *Channel details* and a `curl`-style
post through it lands in the channel.
