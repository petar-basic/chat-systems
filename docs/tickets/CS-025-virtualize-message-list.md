# CS-025 — Virtualize the message list

**Wave:** 6 — Performance
**Area:** frontend
**Blocked by:** CS-024
**Blocks:** —
**Roadmap:** was M11

## Problem

[`MessageList`](../../frontend/src/features/messaging/MessageList.tsx) renders every
loaded message. Pagination bounds what is fetched, but a user who scrolls back through a
busy channel accumulates thousands of mounted rows, and each scroll event re-renders all
of them.

After CS-024 each row is cheap, so this is now a straightforward windowing problem rather
than a symptom of the renderer.

## Approach

Windowed rendering over the existing paginated query, without changing the data layer.

1. **`@tanstack/react-virtual`** with dynamic measurement — message heights vary with
   content, attachments and reaction rows, so fixed-size estimation will not do. Use
   `measureElement` and seed `estimateSize` from a per-message heuristic so the initial
   scrollbar is not wildly wrong.
2. **Anchor scrolling from the bottom.** A chat list grows upward when older pages load
   and downward on new messages. Both must preserve the visual anchor:
   - loading an older page must not move the viewport,
   - a new message while scrolled up must not yank the viewport down; show the existing
     "jump to latest" affordance instead,
   - a new message while at the bottom must stick to the bottom.
   This is the part that breaks in naive implementations. Write these three as explicit
   tests before wiring the virtualizer.
3. **Keep grouping intact.** `messageGrouping.ts` decides whether a message renders with an
   avatar and header or as a continuation. Grouping depends on the *previous* message, so
   the virtualizer must receive a pre-computed flat list of already-grouped rows rather
   than deciding per rendered item. Compute it in the existing hook, memoized on the
   message array.
4. **Preserve deep links and jump-to-message.** Permalink navigation and thread jumps
   currently rely on the target being in the DOM. Route them through
   `scrollToIndex` and, when the target is not in the loaded set, fetch around it first.
5. **Keep the DOM order semantic.** Virtualized lists commonly break text selection across
   items and browser find-in-page. Accept the trade-off consciously, and keep the
   "copy link to message" affordance working since it becomes the practical substitute.
6. **Apply the same treatment to `ConversationView`**, which has the same structure — do
   not fork the implementation, extract the shared list into a component both use.

## Acceptance

- [ ] A channel with 5000 loaded messages keeps re-render under 100 ms.
- [ ] Loading an older page does not shift the viewport.
- [ ] New messages stick to the bottom only when already at the bottom.
- [ ] Permalinks scroll to the target, fetching around it when needed.
- [ ] Grouping, reactions, pins and thread indicators render identically.
- [ ] Channels and conversations share one list implementation.

## Tests

Component tests for the three scroll-anchor behaviours in step 2. An E2E spec that loads a
seeded large channel, scrolls up through several pages and asserts anchor stability. Reuse
the benchmark fixture from CS-024 and record before/after numbers here.
