# CS-063 — Search finds files by name

**Wave:** 22 — finding files
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-064

## Problem

"Where is the Q3 budget spreadsheet somebody sent last month?" Today search matches message
text only. A file posted with no caption, the common case, cannot be found by its name.
`has:file` narrows to messages with attachments, but still by the message's text. A channel's
files panel ([`ChannelFilesPanel.tsx`](../../frontend/src/components/ChannelFilesPanel.tsx))
lists one channel and has no filter. The README admits it: "Search covers message text, not
file names or file contents."

## Approach

### 1. The index

- `files.search_name TEXT GENERATED ALWAYS AS (search_normalize(filename)) STORED`, a
  trigram GIN index on it, and a btree on `(workspace_id, created_at DESC)` for the empty
  query. Trigram rather than a tsvector: file names are `Q3_budget-final(2).xlsx`, not
  sentences, and people type fragments of them.
- `search_normalize` is the function search already depends on
  ([`20240305000027_search_vector_columns.sql`](../../backend/migrations/20240305000027_search_vector_columns.sql)),
  and it is schema-qualified (migration 46) so a generated column can call it.

### 2. The route

- `GET /workspaces/{id}/search/files?q=&from=&in=&type=&after=&before=` returns files with
  their message, channel and uploader.
- **Visibility is the message search's CTE**, the same `visible` set of channels: guests
  held to membership, private channels only for members, DMs only for participants. Move that
  CTE into one SQL function or one shared `WITH` fragment in
  [`messaging/repo.rs`](../../backend/api/src/messaging/repo.rs) so the two searches cannot
  drift. A file whose message was deleted, or that was never attached, is not found.
- `type` narrows by kind, using the categories the frontend already draws icons for in
  [`fileKinds.ts`](../../frontend/src/features/messaging/fileKinds.ts): pdf, image, video,
  audio, document, spreadsheet, presentation, archive, code, other. The backend owns the MIME
  and extension rules and returns `kind` on every file; `fileKinds.ts` keeps only the icon and
  tone per kind, so the filter and the icon cannot disagree.
- Ranking: trigram similarity, then recency. An empty `q` with filters lists the newest
  files, for "every PDF in #legal".

### 3. The UI

- The search panel ([`SearchPanel.tsx`](../../frontend/src/components/SearchPanel.tsx))
  gets **Messages | Files** tabs over the same query and filter bar. `has:` is hidden on the
  Files tab and `type:` appears.
- A file result shows the kind icon or the thumbnail, the name with the match highlighted,
  size, uploader, channel and date. Clicking an image or video opens the gallery; any other
  file jumps to its message. Download is a hover action.
- The channel files panel gets a name filter that calls the same route with `in=`.

## Acceptance

- [ ] A file is found by any fragment of its name, ignoring case and diacritics, in every
      channel and DM the searcher can read, and in no other.
- [ ] Guests find files only in channels they belong to.
- [ ] Filters combine: `from:`, `in:`, `type:`, date range, and an empty query with filters.
- [ ] Results open the gallery or the message; the channel files panel filters by name.
- [ ] The README's search row no longer says file names are not searched.

## Tests

`http_tests`: the authorization matrix plus the visibility cases the message search has
(private channel non-member, guest, DM non-participant, deleted message, unattached upload),
and each filter. A migration test that the generated column fills for existing rows. Frontend:
the tabs, the result row, the panel filter. E2E: upload `Q3 budget.xlsx` with no caption and
find it by typing `budg`.
