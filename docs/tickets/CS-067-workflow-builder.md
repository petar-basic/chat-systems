# CS-067 — Workflow builder

**Wave:** 23 — structured work: polls, lists, workflows
**Area:** backend · frontend · ops
**Blocked by:** CS-065 (the interactive-message pattern), CS-066 for part 4
**Blocks:** —

## Problem

The repetitive coordination work in a team, written by somebody who is not a developer:
greet whoever joins `#general` and send them the onboarding links, post "standup in 10" every
weekday at 9:50, turn a 🚨 reaction into a post in `#incidents`, collect a time-off request
through a form and drop it in a list. Slack's Workflow Builder does this without code, and
for many teams it is the reason they stay on a paid plan.

What exists here is for developers: incoming and outgoing webhooks, slash commands and
connected apps ([`hooks/`](../../backend/api/src/hooks/)). Two members of the `hook_type`
enum, `bot` and `scheduled`, were declared in the initial schema and are used nowhere; this
ticket removes them rather than building beside them.

## Approach

Four parts, each its own PR, in this order.

### 1. The engine, two triggers, two steps, and the builder ✅ done

Landed as described, with these differences:

- Validation is a typed `Definition` (`serde`-tagged triggers, steps and recipients) with its
  own check, not `garde`: the messages name the step ("Step 2: {{webhook.x}} isn't available
  for this trigger"). Saving also checks the timezone against Postgres and, as the builder,
  each step's channel and person, so a step the builder could not do by hand is refused then.
- A new workflow is a draft; *Publish* turns it on. Editing a published one keeps it on and
  bumps `version`. Only its builder edits or publishes it; an admin sees, turns off and deletes
  anyone's.
- A schedule's next instant is computed in Postgres (`workflow_next_run()`, DST-correct), kept
  in `workflows.next_run_at`, and queued as a run keyed `schedule:<instant>`. One missed by
  more than 15 minutes (the worker was down) is skipped rather than posted late.
- The runner polls every 5 seconds for queued runs. Each step's message and its
  `workflow_run_steps` row commit together, so a run picked up again after a crash never
  repeats a step. A run left `running` for 5 minutes is picked up again.
- A webhook call's run key is its `Idempotency-Key` header, or a fresh id without one; it
  answers 202 with the run id, 409 while the workflow is off, and 429 above 30 calls a minute.
  A variable is any JSON value, inserted as text.
- Mentions written into a step's text notify; a mention arriving inside a webhook value is
  posted but notifies nobody.
- A DM goes to the conversation between the builder and the person, posted as the workflow.
  A workflow cannot DM its own builder.
- `{{trigger.user}}`, `{{trigger.channel}}` and the trigger-person recipient exist in the
  definition but no part-1 trigger provides them; part 2's triggers do.

- **A workflow is data, validated by types.**
  `workflows (id, workspace_id, name, description, created_by, definition JSONB, version,
  enabled, created_at, updated_at)`. The `definition` deserializes into a tagged Rust enum per
  trigger and per step, validated with `garde` on save. An invalid definition is refused at
  save time, never discovered at run time.
- **Triggers in this part:** `schedule` (daily, weekdays, or chosen weekdays at a time, in
  a timezone) and `webhook` (a secret URL with declared text variables, rate-limited like
  incoming hooks, [`hooks/routes.rs`](../../backend/api/src/hooks/routes.rs)).
- **Steps in this part:** `send_message` (to a channel) and `send_dm` (to a person or to the
  person who triggered it), with text that can insert variables: `{{trigger.user}}`,
  `{{trigger.channel}}`, `{{webhook.<name>}}`. A person variable renders as a real mention.
- **The runner** is a supervised `workflow_runner` in `chat-worker`. Every run is a row,
  `workflow_runs (id, workflow_id, workflow_version, trigger_key, status, context JSONB,
  error, started_at, finished_at)`, unique on `(workflow_id, trigger_key)`. The trigger key
  is the scheduled instant, the webhook request id, or the stream event id, so at-least-once
  delivery never runs a step twice. Steps record their outcome in `workflow_run_steps`. A
  failed step stops the run and says why in the history.
- **Whose authority a workflow acts with.** It posts as a bot message
  (`create_bot_message_in`, with `metadata.bot.workflow_id` and the workflow's name), but
  every step is authorized **as its creator, at run time**: `require_channel_post` for the
  target channel, workspace membership for a DM. A creator who loses access to a channel
  makes that step fail. A creator who leaves the workspace disables the workflow, and the
  admins get an Activity row.
- **Loop protection.** Messages posted by a workflow never trigger workflows, and every
  workflow is rate-limited per minute.
- **Who builds.** A workspace setting `workflow_builders: members | admins`, default
  `members`. Admins see, disable and delete every workflow; the audit log records create,
  edit, enable, disable and delete.
- **The builder.** "Workflows" in the workspace menu opens a list (name, trigger, enabled,
  last run, owner). The editor is a full-height dialog: one trigger card, then the step
  cards in order, each a small form built from CS-059 fields, with an "Insert variable" menu
  in text fields. Publish, Disable, and **Run history** (the last 50 runs with each step's
  outcome) round it out.
- Migration: drop `bot` and `scheduled` from `hook_type`.

### 2. Event triggers

- `reaction_added` (an emoji in a chosen channel) and `member_joined` (a chosen channel).
  The runner reads them from the workspace streams through its own `StreamGroup`, the same
  way the notification consumer does. `{{trigger.message}}` and `{{trigger.message_link}}`
  become available for reactions.
- A trigger on a private channel requires the creator to be a member when saving and
  re-checks at run time.

### 3. Shortcuts and forms

- **A `shortcut` trigger** shows the workflow in a channel's composer menu and as
  `/workflow <name>` there.
- **A `form` step** sends the trigger person (or a named person) a message with a "Fill
  in" button: the interactive-message pattern from CS-065. The button opens a `Modal`
  built from the step's fields (short text, long text, select, person, date). Submitting
  stores the answers in the run's `context` as `{{form.<field>}}` and resumes the run, so a
  run can now wait. Waiting runs expire after 7 days with status `expired`.
- **A `send_message` option** posts the form answers as a formatted summary.

### 4. Lists

- **An `add_list_item` step** maps variables to the columns of a chosen list (CS-066),
  validated against the column kinds at save time. **An `update_list_item` step** fills
  a person or status column on the item a previous step created.
- Templates in the "New workflow" dialog: welcome new members, daily standup reminder,
  incident from a reaction, time-off request into a list.

## Acceptance

- [ ] A member builds, publishes and disables a workflow without leaving the app;
      admins can restrict building to admins.
- [ ] Schedule, webhook, reaction, member-joined and shortcut triggers start runs; each
      trigger starts exactly one run even when delivered twice.
- [ ] Send message, send DM, form and list steps work with variables; forms pause the run
      until answered.
- [ ] A step never does what its creator could not do by hand at that moment.
- [ ] Workflow messages never trigger workflows; a busy trigger is rate-limited.
- [ ] Run history shows every run and the failing step's reason.
- [ ] `bot` and `scheduled` are gone from `hook_type`.

## Tests

Definition validation unit tests: every trigger and step, the bad cases. `http_tests` for the
CRUD routes, the builder setting and the webhook trigger (bad token 401, rate limit 429).
Runner tests per trigger: one run per trigger key under redelivery, authorization as the
creator re-checked at run time, the creator leaving the workspace, loop protection, form
pause, resume and expiry. Frontend: the editor (adding, reordering and removing steps,
variable insertion), the run history, and the form modal from the button. E2E: a
member-joined workflow welcomes a new member with a DM.
