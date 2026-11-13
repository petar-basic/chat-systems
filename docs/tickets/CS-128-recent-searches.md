# CS-128 — Recent searches

**Wave:** 34 — writing and finding (2026-11-13)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

Every search starts from an empty box. Somebody who looks for `deploy in:#ops` every morning
types it every morning, filters included. Slack lists recent searches when the box opens.

## Approach

- **A `recent_searches` table:**
  - `(user_id, workspace_id, query, filters jsonb, searched_at)`.
  - `filters` is what `SearchFilterBar` holds: person, channel, `has`, dates and the files
    tab.
  - A search counts when it is submitted or its results are opened, not on each keystroke.
  - The same query with the same filters moves to the top instead of repeating.
  - The newest 20 per person and workspace are kept.
- **Routes:** list, delete one, and clear all, under `/workspaces/{ws_id}/searches/recent`,
  for the person's own searches only.
- **The search box,** focused and empty, lists them under *Recent*.
  - Each shows the filters as chips. Picking one runs it with those filters.
  - Each has a remove button; *Clear* at the end removes all.
  - Arrow keys and Enter work as in the result list.
- **Privacy:** a recent search names a channel by id. If you lose access to the channel, its
  name stops showing and the search runs without that filter.
- **Export** includes them and **erase** removes them (CS-121j).

## Acceptance

- [ ] Search `deploy` filtered to `#ops`, then reopen the box: the search is first under
  *Recent*, chips included, and picking it gives the same results.
- [ ] A search repeated ten times shows once.
- [ ] Removing one and *Clear* both work, and survive a reload and another device.
- [ ] A recent search naming a private channel you left shows no channel name.

## Tests

`http_tests`:
- recording, de-duplication and the cap of 20;
- the routes' authorization matrix, where another person's list answers nothing;
- a channel lost after a search;
- export and erase.

Frontend: the *Recent* list with keyboard navigation.
