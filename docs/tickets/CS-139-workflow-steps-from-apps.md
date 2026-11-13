# CS-139 — Workflow steps from apps

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend · docs
**Blocked by:** CS-138
**Blocks:** —

## Problem

A workflow can only do what the app ships, and a team's own systems are out of its reach:
the ticketing tool, the HR system, the deploy pipeline. The webhook trigger lets them start
a workflow but not take part in one. In Slack an app offers custom steps, which appear in
Workflow Builder next to the built-in ones.

## Approach

- **Step definitions** on the app's page, or sent by the app through a route, each with:
  - a `callback_id`, a title and a description;
  - typed inputs and outputs. The types are text, number, boolean, person, channel and
    date, and each input says whether it is required.
  - At most 20 steps per app.
- **In the builder:**
  - The steps of the workspace's enabled apps are listed under the app's name.
  - Inputs are filled with fixed values or variables from earlier steps, checked against
    their types.
  - Outputs become variables for later steps and branches (CS-138).
- **Running:**
  - The runner sends `function_executed` with the run id, the step's `callback_id` and the
    inputs, over HTTP or the app's socket like any event. The run then waits as a form step
    does.
  - The app finishes the step through a route, with outputs that are checked against the
    definition, or fails it with a message the run history shows.
  - A step that gets no answer in one hour fails as *The app did not finish*.
- **The scope** is new: `workflow.steps:execute`.
- **When an app is disabled or removed:** its steps fail when they are reached, workflows
  using them show a warning in the builder, and the workflow's owner gets an Activity item
  once.
- **`docs/bot-api.md`** documents the definitions, the event and the completion route, with
  a step that opens a ticket and returns its id.

## Acceptance

- [ ] A test app defines *Open ticket* (input title, output ticket id). A workflow uses it
  and posts *Ticket {id} opened*, and the run history shows the step's inputs and outputs.
- [ ] Outputs of the wrong type are refused, and the run waits until a valid completion or
  the hour runs out.
- [ ] The app failing the step shows its message in the run history, and later steps do not
  run.
- [ ] A disabled app's step fails with a clear error, and the builder warns.

## Tests

`http_tests`:
- definitions and their validation;
- the event over HTTP and over a socket;
- completion and failure;
- the timeout through the loop;
- the scope check;
- a disabled app.

E2E: build a workflow with a test app's step and run it.
