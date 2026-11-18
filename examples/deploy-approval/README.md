# Deploy approval

An app that asks for a deploy in a modal and lets the channel approve it with a button. It is
one file, runs on Node 22 with no dependencies, uses socket mode so it needs no public URL,
and shows every part of [interactivity](../../docs/bot-api.md#buttons-menus-and-modals):

- `/request` opens a modal with the command's `trigger_id`;
- the modal's *Add release notes* button pushes a second view on top of it, and the first keeps
  what was typed;
- a service name it does not like comes back as an error under that input;
- a valid request closes the modal and posts a message with *Approve* and *Reject*;
- a click on either reaches the app, which replaces the message with *Approved by …* through
  the click's `response_url`.

1. In *Workspace settings → Apps → Build an app*, create an app with `chat:write` and
   `commands`, agree to them, and copy its token.
2. Open its page in the directory, tick *Socket mode* and *Interactivity*, *Save*, then *Make an
   app-level token* and copy it.
3. Add the bot to a channel, and register its command there as a `slash_command` hook owned
   by the app, with no URL:

   ```bash
   curl -s -X POST https://chat.example.com/api/workspaces/$WORKSPACE_ID/hooks \
     -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
     -d '{ "hook_type": "slash_command", "name": "request", "app_id": "'$APP_ID'",
           "config": { "command": "request", "channel_ids": ["'$CHANNEL_ID'"] } }'
   ```

4. Run it:

   ```bash
   CHAT_API_URL=https://chat.example.com/api CHAT_BOT_TOKEN=cbot-… CHAT_APP_TOKEN=capp-… node deploy-approval.mjs
   ```

Type `/request` in that channel. `--once` exits after the first approval or rejection; the
end-to-end suite runs it that way.
