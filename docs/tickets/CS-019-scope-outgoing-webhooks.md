# CS-019 — Scope outgoing webhooks per channel

**Wave:** 4 — Governance
**Area:** backend/api · frontend
**Blocked by:** CS-004 (the consumer moves to the worker), CS-018 (audit the change)
**Blocks:** —
**Audit finding:** S8 (MEDIUM)

## Problem

[`start_hook_consumer`](../../backend/api/src/hooks/executor.rs#L88) reacts to every
`message.created` in a workspace by listing all active outgoing hooks and dispatching to
each:

```rust
let hooks = hook_repo.list_active_outgoing_hooks(ws_id).await?;
for hook in hooks {
    dispatch_hook(&http, &hook_repo, &hook, &event_type, &event_payload).await;
}
```

There is no channel filter anywhere in that path. So a single outgoing webhook created by
any Workspace Admin streams the full text of **every message in every channel**,
including private ones, to an external URL. Nobody in those channels is told.

The transport itself is well built — SSRF validation with DNS resolution checks, redirects
disabled, HMAC signature, bounded retries
([`hooks/ssrf.rs`](../../backend/api/src/hooks/ssrf.rs)). The problem is scope and
visibility, not delivery.

Slack scopes outgoing webhooks to a channel and shows an integration in the channel's
member list. This should do the same.

## Approach

1. **Scope is required, not optional.** `config.channel_ids` becomes mandatory for
   `HookType::OutgoingWebhook` at creation, validated the same way
   `incoming_webhook` already validates its single `channel_id`
   ([`hooks/routes.rs:70`](../../backend/api/src/hooks/routes.rs#L70)): each channel must
   exist and belong to the workspace.
2. **Filter at the query, not in the loop.** New repo method
   `list_active_outgoing_hooks_for_channel(workspace_id, channel_id)` with the channel
   predicate in SQL:
   ```sql
   WHERE workspace_id = $1
     AND hook_type = 'outgoing_webhook'
     AND is_active
     AND config->'channel_ids' ? $2::text
   ```
   Add a GIN index on `(config)` if the hook count ever justifies it; at current scale a
   sequential scan over a handful of rows is fine and simpler.
3. **Migration for existing hooks.** There is no correct automatic answer — an existing
   hook was created with workspace-wide scope and its owner may depend on it. Deactivate
   them (`is_active = false`) and record an `HookDeleted`-adjacent audit entry, so an
   admin has to consciously re-enable with an explicit scope. Note it in `RUNBOOK.md` as a
   breaking upgrade step. Silently narrowing the scope would break integrations without
   telling anyone; silently keeping it would leave the hole open.
4. **Make it visible in the channel.** A channel with an active outgoing hook shows an
   integration indicator in the members panel and a system message when one is attached or
   detached. The people whose messages leave the building should be able to see that they
   do.
5. **Restrict who can attach.** Creating a hook already requires `WorkspaceRole::Admin`
   ([`hooks/routes.rs:67`](../../backend/api/src/hooks/routes.rs#L67)). Additionally
   require channel-moderator rights on each scoped private channel, so a workspace admin
   cannot silently tap a private channel they are not in.
6. **Payload minimisation.** The hook currently receives the whole message event. Send
   the fields an integration actually needs (`id`, `channel_id`, `user_id`, `content`,
   `created_at`) rather than forwarding the internal event verbatim, so future fields do
   not leak by default.

## Acceptance

- [ ] An outgoing webhook cannot be created without at least one channel id.
- [ ] Messages from unscoped channels are never dispatched.
- [ ] Existing hooks are deactivated by the migration and the upgrade note documents it.
- [ ] Attaching a hook to a private channel requires moderator rights on that channel.
- [ ] Channel members can see that an integration is attached.
- [ ] Hook create, delete and scope change are audited.

## Tests

`http_tests/hooks.rs`: create a hook scoped to channel A, post in A and in B, assert one
delivery. Create without `channel_ids` → 400. Attach to a private channel as a
non-moderator admin → 403. Assert the dispatched payload contains only the allowlisted
fields.
