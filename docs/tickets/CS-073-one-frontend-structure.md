# CS-073 — One frontend structure

**Wave:** 24 — ready for a company
**Area:** frontend
**Blocked by:** CS-070 (it moves the admin panels first)
**Blocks:** —

## Problem

The frontend has two layouts at once. Newer work lives in `src/features/<feature>/`; older
work is still in `src/components/` (23 panels: search, threads, settings, members, pinned
messages and more) and `src/hooks/queries/` (24 query hooks), with `src/pages/` beside them.
Finding where something lives depends on when it was written. Two components carry far more
than one job: `MessageItem.tsx` (806 lines) and `HuddleWindow.tsx` (643).

## Approach

One PR per group of moves, no behaviour change in any of them.

- Every file in `components/` and `hooks/queries/` moves into the feature it belongs to
  (search, threads, channel, notifications, later, integrations, huddle, …), and its tests
  with it. What is genuinely shared goes to `shared/`. `components/` and `hooks/` are deleted.
- `MessageItem` splits into the row, its actions bar, its body (content, attachments, unfurls,
  poll, form) and its edit mode; `HuddleWindow` into the stage, the participant tiles and the
  controls.
- A lint rule (`no-restricted-imports` plus a path check) refuses new files outside
  `features/`, `shared/`, `pages/`, `lib/`, `stores/` and `api/`, and refuses a feature
  importing another feature's internals except through its `index.ts`.
- `docs/frontend.md` describes the layout once, at the top.

## Acceptance

- [ ] `src/components/` and `src/hooks/` no longer exist.
- [ ] `MessageItem` and `HuddleWindow` are each split into parts of one job.
- [ ] The lint rule fails a file added in the wrong place.

## Tests

The moved tests pass unchanged. `tsc -b`, the lint rule, and the full E2E run are the check
that nothing changed.
