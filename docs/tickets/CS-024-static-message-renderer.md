# CS-024 — Replace the per-message editor with a static renderer

**Wave:** 6 — Performance
**Area:** frontend
**Blocked by:** —
**Blocks:** CS-025, CS-029
**Audit finding:** P1 (HIGH)

## Problem

[`RichTextDisplay`](../../frontend/src/components/RichTextDisplay.tsx#L30) calls
`useEditor` **once per message**:

```tsx
const editor = useEditor({ editable: false, extensions, content: displayContent || '' });
```

Every rendered message therefore mounts a full TipTap instance — a ProseMirror
`EditorState`, an `EditorView`, a plugin stack, a schema and a contenteditable DOM subtree
— to display text that is never edited. A channel with 300 loaded messages holds 300 of
them, and `MessageList` renders every loaded message
([`MessageList.tsx`](../../frontend/src/features/messaging/MessageList.tsx)).

This is why the list gets slow, and it is the reason virtualization alone is not the fix:
windowing reduces how many editors are mounted, but mounting and unmounting an editor
during scroll is far more expensive than mounting a `<span>`. The renderer has to change
first; then windowing is worth doing.

## Why before CS-025

Virtualizing a list of editor instances optimizes the wrong layer, and the two changes
touch the same components. Done in this order, CS-025 is a small ticket. Done in the
other order, it is done twice.

## Approach

Render messages from a parsed representation instead of an editor. Keep TipTap for the
composer, where it is the right tool.

1. **Parse once, at the edge.** Message content is markdown produced by
   `tiptap-markdown` with `html: false`
   ([`tiptapExtensions.ts`](../../frontend/src/lib/tiptapExtensions.ts#L36)). Parse it to a
   plain node tree with a small markdown parser configured to the same subset the composer
   can produce — paragraph, bold, italic, strike, underline, code, code block, blockquote,
   bullet and ordered list, link, hard break. Nothing else needs to round-trip, because
   nothing else can be authored.
2. **Render the tree to React elements.** A `MessageContent` component walking the tree,
   memoized on the raw content string. No `dangerouslySetInnerHTML` anywhere — the tree
   maps to elements directly, which preserves the current XSS posture by construction
   rather than by configuration.
3. **Keep the link policy identical**: protocols limited to `http`, `https`, `mailto`;
   `rel="noopener noreferrer nofollow"`; `target="_blank"`. Anything outside the allowlist
   renders as plain text, not as an anchor. This is the one place a regression would be a
   security regression, so port the rule deliberately and test it.
4. **Mentions stay a post-parse decoration.** `parseMentions` / `flattenMentions`
   ([`lib/mentions.ts`](../../frontend/src/lib/mentions.ts)) already produce the label set
   the highlighter needs; apply the highlight while walking text nodes instead of through a
   ProseMirror plugin. Self-mention styling continues to depend on the current user id.
5. **Attachment cards are unchanged** — `parseAttachment` short-circuits before the
   markdown path today and should keep doing so.
6. **Editing keeps TipTap.** `MessageItem` already swaps in `MessageInput` for the edit
   state; that is exactly one editor instance at a time, which is correct.
7. **Delete the display half of the TipTap configuration** (`createDisplayExtensions` and
   `mentionHighlightExtension`) once nothing imports them, so there is one rendering path,
   not two.

## Acceptance

- [ ] No `useEditor` call outside the composer and the inline edit form.
- [ ] Rendered output is visually identical for every construct the composer can produce —
      verify against a fixture of representative messages.
- [ ] `javascript:` and `data:` URLs render as text, not anchors.
- [ ] Mention highlighting, including self-mentions, is unchanged.
- [ ] A 500-message channel drops measurably in mount time and memory; record the numbers
      here.

## Tests

Component tests for `MessageContent` covering each supported construct, the link allowlist
and mention highlighting. Extend the existing hostile-payload E2E spec — it is the
regression test for step 3. Add a rendering benchmark fixture so CS-025 can be measured
against a known baseline.
