# Ticketing step

An app that adds an *Open ticket* step to workflows. It is one file, runs on Node 22 with no
dependencies, uses socket mode so it needs no public URL, and shows the
[workflow steps](../../docs/bot-api.md#workflow-steps) API:

- on start it defines its step with `PUT /bot-apps/me/workflow-steps`: a required *Title*, an
  optional *Reporter*, and a *Ticket ID* it hands back;
- a run that gets to the step sends the app `function_executed` with the inputs, and the app
  finishes it with the new ticket's id, which later steps use as `{{step.N.ticket_id}}`;
- a title that says *help desk is down* fails the step instead, and the run history shows
  why.

1. In *Workspace settings → Apps → Build an app*, create an app with `workflow.steps:execute`,
   agree to it, and copy its token.
2. Open its page in the directory. Tick *Socket mode* and the `function_executed` event,
   *Save*, then *Make an app-level token* and copy it.
3. Run it:

   ```bash
   CHAT_API_URL=https://chat.example.com/api/v1 CHAT_BOT_TOKEN=cbot-… CHAT_APP_TOKEN=capp-… node ticketing.mjs
   ```

In *Workflows*, add a step, pick *Open ticket* under the app's name, fill in *Title*, and add a
message after it with *Step 1: Ticket ID* from *Insert variable*.
