# CS-078 — A suspended or removed person stops acting

**Wave:** 26
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

Admin *Suspend* revokes sessions and app connections, but nothing else: the person's
scheduled messages still send, their workflows still run, and their outgoing webhooks keep
posting the content of private channels to a third-party URL. None of the executors —
scheduled messages, the workflow runner, outgoing hooks — checks the author's status or
access when it fires. SCIM deactivation goes further (`membership::detach`), but detach does
not touch hooks either. When a workflow's creator leaves, only they could edit or turn it
on, so nobody can.

## Approach

- One check at fire time, `authz::still_acting(state, ws_id, user_id, channel_id?)`: the
  account is active, a member of the workspace, and (for a channel) still has access. The
  scheduled executor, the workflow runner and outgoing-hook delivery call it and skip, with
  the reason recorded (`failed` with `author_inactive` for a scheduled message, a run error
  for a workflow, a disabled hook with a reason).
- Suspend and erase also turn off the person's outgoing hooks and workflows, audited; activate
  does not turn them back on.
- A workflow whose creator left can be taken over by a workspace admin (*Take over* in the
  *Workflows* panel, `POST /workflows/{id}/owner`), audited.

## Acceptance

- [ ] After suspend, nothing the person set up posts or sends anywhere.
- [ ] Losing access to a private channel stops the hooks and workflows that read it.
- [ ] An admin can take over a workflow whose creator left.

## Tests

Each executor with a suspended author, a removed member and a lost private channel; hook
disabled on suspend; takeover permissions and audit row.
