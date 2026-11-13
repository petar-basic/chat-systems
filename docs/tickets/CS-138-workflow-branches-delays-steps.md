# CS-138 — Workflows that branch, wait and do more

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-139

## Problem

A workflow (CS-067) has five triggers and five steps:
- triggers: schedule, webhook, reaction, member joined, shortcut;
- steps: send a message, send a DM, a form, add a list item, update a list item.

It runs top to bottom. It cannot do *if the form says urgent, page on-call, otherwise file
it*. It cannot wait *a day after somebody joins, send the checklist*. It cannot make the
channel for a new project. Slack's Workflow Builder can do all three.

## Approach

- **A branch step:**
  - It tests one value against a condition: equals, contains, is empty, greater than, or
    one of a list. The value is a form answer, a trigger variable, the trigger user's
    profile field (CS-126) or a list column.
  - Each branch is a list of steps, and the run joins up after the branch.
  - Branches can nest two deep.
  - The builder draws them as indented lanes, with the *Add step* of `StepCard`.
- **A delay step:** wait a number of minutes, hours or days, or until a date and time in
  the workflow's time zone.
  - The run waits the way a form step does (`Outcome::Waiting`) with a `resume_at`. A
    `chat-worker` loop resumes due runs, claiming each once.
  - Delays are at most 30 days.
- **New steps,** each run as the workflow's owner and held to the owner's rights at run time
  (CS-121c):
  - add a reaction to the triggering message;
  - reply in the triggering message's thread;
  - create a channel, public or private, under the owner's channel-creation rules (CS-075);
  - add people to a channel, from a form answer, the trigger user or a group;
  - set a channel's topic;
  - archive a channel.
  - Each step's outputs (the new channel, the reply) become variables for the steps after
    it.
- **Run history** (`/workflows/{id}/runs`) shows which branch ran, where a run is waiting
  and until when, and the step that failed with its error.
- **Editing a workflow with waiting runs:** they finish on the version they started on,
  which `workflow.version` already records.

## Acceptance

- [ ] A form with *Urgency* sends an urgent answer to `#oncall` and any other answer to
  `#requests`, and the run history shows the branch.
- [ ] *Member joined → wait 1 day → DM the checklist*, with the clock moved in the test,
  sends the DM a day later and not before.
- [ ] A workflow creates `#proj-{answer}`, adds the people picked in its form, and posts the
  channel's link back to the trigger user.
- [ ] An owner who loses the right to create channels makes that step fail with a clear
  error in the run history, and nothing is created.
- [ ] Waiting runs survive a worker restart and resume once.

## Tests

`http_tests`:
- branch evaluation for each condition;
- nesting;
- delays resumed by the loop with the clock moved, exactly once across two workers;
- each new step, including its authorization at run time;
- versioned runs.

Frontend: the builder's lanes and validation. E2E: build and run a branching workflow.
