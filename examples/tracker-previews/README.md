# Tracker previews

An app that previews links to its own issue tracker. It is one file, runs on Node 22 with no
dependencies, uses socket mode so it needs no public URL, and shows the
[link preview](../../docs/bot-api.md#link-previews) API:

- the app claims `tracker.example.com`, so a link there gets no page preview of its own;
- posting such a link where the bot is a member sends the app `link_shared` with the message
  and its links, and the app answers with a header and two fields per issue;
- an edit that adds a link asks the app about the new link only; a link where the bot is not a
  member gets no preview at all.

1. In *Workspace settings → Apps → Build an app*, create an app with `chat:write`, `links:read`
   and `links:write`, agree to it, and copy its token.
2. Open its page in the directory. Tick *Socket mode* and the `link_shared` event, *Save*, then
   *Make an app-level token* and copy it.
3. Under *Link previews*, add the domain `tracker.example.com` and *Save domains*.
4. Add the bot to the channels where its previews should show.
5. Run it:

   ```bash
   CHAT_API_URL=https://chat.example.com/api CHAT_BOT_TOKEN=cbot-… CHAT_APP_TOKEN=capp-… node tracker.mjs
   ```

Post `https://tracker.example.com/issues/7` in one of those channels to see its preview.
