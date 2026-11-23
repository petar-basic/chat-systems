# Help desk

An app with a Home tab and two shortcuts. It is one file, runs on Node 22 with no dependencies,
uses socket mode so it needs no public URL, and shows the parts of the
[App Home and shortcuts](../../docs/bot-api.md#app-home-and-shortcuts) API:

- opening the app's Home tab sends `app_home_opened`, and the app publishes that person's own
  requests with `views.publish`; nobody else sees them;
- *New request*, a global shortcut in the composer's shortcut menu and the quick switcher, opens
  the app's modal, and so does the *New request* button in the Home;
- *Create ticket from message*, a message shortcut under *More actions*, opens a modal with the
  message's text already in it, then says in the channel that a ticket was opened;
- each request sent republishes the sender's Home, which updates without a reload.

1. In *Workspace settings → Apps → Build an app*, create an app with `chat:write`, agree to it,
   and copy its token.
2. Open its page in the directory. Tick *Socket mode*, *Interactivity*, *Home tab* and the
   `app_home_opened` event, *Save*, then *Make an app-level token* and copy it.
3. Under *Shortcuts*, add a global shortcut with the callback ID `new_request` and a message
   shortcut with the callback ID `ticket_from_message`, and *Save shortcuts*.
4. Add the bot to the channels where its message shortcut should show.
5. Run it:

   ```bash
   CHAT_API_URL=https://chat.example.com/api/v1 CHAT_BOT_TOKEN=cbot-… CHAT_APP_TOKEN=capp-… node helpdesk.mjs
   ```

Open a conversation with the app to see its Home. `--once` exits after three requests; the
end-to-end suite runs it that way.
