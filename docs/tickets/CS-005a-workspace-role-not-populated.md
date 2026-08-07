# CS-005a — Role-gated UI disappears when the workspace role is not populated

**Wave:** 0 — tail (found while turning the E2E suite on; blocks its acceptance)
**Area:** frontend
**Blocked by:** —
**Blocks:** making the `e2e` job a blocking check (CS-001's acceptance)
**Found:** 2026-08-07, by the first CI run of the Playwright suite

## Problem

A user who holds a role intermittently loses every control that role unlocks. It is
not a rendering delay — the data arrives correct and the UI never catches up.

**Proven, from a CI a11y snapshot.** One render, one user (workspace **owner**), the
workspace menu open:

```
- button "Members"
- button "Settings"
- button "Scheduled"
- button "Instance Admin"     ← gated by isInstanceAdmin   → shown
  (no Integrations entry)     ← gated by isWorkspaceAdmin  → hidden
```

`isInstanceAdmin` comes from `user.is_instance_admin`; `isWorkspaceAdmin` is
`currentUserRole === 'admin' || 'owner'`
([`ChannelSidebar.tsx:185`](../../frontend/src/features/channel/ChannelSidebar.tsx#L185)).
Same component, same commit, same instant: one is populated, the other is `null`.

**Proven, from the CI trace.** In the `channel-permissions` failure the browser did
receive the correct data:

| time | request | body |
|---|---|---|
| 07:57:53.697 | `GET /channels/786a…/members` | `9dd706b3=member` |
| 07:57:53.759 | `PATCH …/members/9dd706b3/role` | 200 |
| **07:57:54.168** | `GET /channels/786a…/members` | **`9dd706b3=admin`** |

The assertion polled until 07:58:08.8 — **14.6 seconds after the correct response
landed** — and the settings control never appeared. The UI does not re-render off that
data.

**Reproducible locally**, so this is not a CI-only artifact: 3 failures in 8 runs of
`channel-permissions.spec.ts:22`, 1 in 6 of `integrations.spec.ts:62`.

### What it costs

- A channel admin cannot open channel settings (the gear is absent).
- A workspace admin/owner cannot reach Integrations.
- [`MembersPanel.tsx:132`](../../frontend/src/components/MembersPanel.tsx#L132) reads the
  same value to decide which member actions to offer.
- [`wsQuerySync.ts:49`](../../frontend/src/lib/wsQuerySync.ts#L49) reads it via
  `getState()` to decide whether to suppress a notification, so a stale value changes
  notification behaviour too.

Authorization itself is not bypassed — the API still enforces (`authz`). This is the UI
denying an action the user is entitled to, which reads to them as a broken product.

## Where to look

Both values are set by two effects in
[`useWorkspaceController.ts:164-172`](../../frontend/src/features/workspace/hooks/useWorkspaceController.ts#L164-L172):

```ts
useEffect(() => {
  if (!user || workspaceMembers.length === 0) return;
  const mine = workspaceMembers.find((m) => m.user_id === user.id);
  setCurrentUserRole(mine ? (mine.role as WorkspaceRole) : null);
}, [workspaceMembers, user, setCurrentUserRole]);

useEffect(() => {
  setCurrentUserId(user?.id ?? null);
}, [user?.id, setCurrentUserId]);
```

and `user` does **not** come from a query. `useCurrentUser`
([`useAuth.ts:9-20`](../../frontend/src/hooks/queries/useAuth.ts#L9-L20)) is a `useMemo`
over the instance store, which `restoreInstances()` fills asynchronously after
`/users/me` (or `/auth/refresh`) resolves. So there is a window on every load where
`user` is `null`, `currentUserId` is explicitly set to `null`, and the role effect
bails out.

**Leading hypothesis — not yet proven:** the two sources settle in an order that leaves
the store unset, and nothing re-triggers the effect afterwards because neither
`workspaceMembers` (already cached and stable) nor `user` changes identity again.
Whoever picks this up should confirm the mechanism before changing anything; a fix aimed
at the wrong ordering will look like it works and reappear as a flake.

### A second defect in the same effect

The `workspaceMembers.length === 0` guard means the role is never *cleared* on a
workspace switch. Switching from a workspace you own to one where you are a guest keeps
the previous `owner` value until the new members list arrives — so admin controls are
briefly offered in a workspace where they do not apply. Clicking them fails at the API,
but the UI should not have shown them. Fix both in one pass.

## Approach

1. **Confirm the mechanism first.** Instrument the two effects, reproduce with
   `npx playwright test e2e/channel-permissions.spec.ts:22 --repeat-each=8`, and capture
   the ordering of `user`, `workspaceMembers` and the resulting store writes. Do not
   skip to a fix.
2. **Derive, do not synchronise.** The root problem is that a value derivable from two
   queries is copied into Zustand by effects, so it can be stale or unset and nothing
   recomputes it. Prefer a selector/hook that computes the role from the live query data
   on every render:
   ```ts
   export function useCurrentWorkspaceRole(): WorkspaceRole | null
   ```
   backed by `useCurrentUser()` + `useWorkspaceMembers(activeWorkspaceId)`, with the
   store keeping only what genuinely needs to be global.
   `wsQuerySync` reads it outside React via `getState()`, so it still needs a store
   mirror — write that mirror from one place, not from a component effect.
3. **Distinguish "not loaded yet" from "no role".** Today both are `null`, which is why
   the UI silently degrades to no-permissions instead of showing a loading state. Make
   the loading case explicit so a control can render disabled rather than vanish.
4. **Clear on workspace switch** so a stale role from the previous workspace cannot leak
   into the new one.
5. **Then flip the `e2e` job to blocking.** It is the acceptance criterion CS-001 could
   not meet because of this bug.

## Acceptance

- [ ] The mechanism is confirmed and written down here before the fix lands.
- [ ] A channel admin sees the settings control on first paint after a reload, every time.
- [ ] A workspace admin/owner sees the Integrations entry, every time.
- [ ] Switching to a workspace with a lower role never shows the previous workspace's
      controls.
- [ ] `channel-permissions.spec.ts:22` and `integrations.spec.ts:62` pass 20 consecutive
      runs (`--repeat-each=20`).
- [ ] The `e2e` job blocks merges again.

## Tests

Component tests for the role hook covering: user-not-loaded, members-not-loaded, both
loaded, and a workspace switch that lowers the role. Keep the two E2E specs exactly as
they are — they are correct as written and are the regression net. **Do not add a
`waitForResponse` to make them pass**: it does make them green (verified, 8/8), and it
hides this bug rather than fixing it.
