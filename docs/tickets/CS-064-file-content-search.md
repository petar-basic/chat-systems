# CS-064 — Search inside files

**Wave:** 22 — finding files
**Area:** backend · frontend · ops
**Blocked by:** CS-063
**Blocks:** —

## Problem

After CS-063 a file is found by its name. The contract called `Agreement_v7.pdf` is still
not found by searching the client's name written inside it. Slack searches the text of PDFs
and Office documents, and a team that moved its documents into chat expects the same.

## Approach

### 1. Extraction, in the worker

- `file_texts (file_id PRIMARY KEY REFERENCES files ON DELETE CASCADE, content TEXT,
  search_vector TSVECTOR, extracted_at, status)`: a separate table, so the hot `files` row
  and every query that reads it stay small. `status` is `done`, `empty`, `unsupported`,
  `too_large` or `failed`, so the backfill never retries a file it cannot read.
- A supervised `file_text_extractor` in `chat-worker`
  ([`bin/chat-worker.rs`](../../backend/api/src/bin/chat-worker.rs)) claims files with no
  `file_texts` row (`FOR UPDATE SKIP LOCKED`, batches of 20), reads the object through
  `FileStorage`, extracts, and writes the row. The same loop is the backfill for files that
  existed before the migration; no separate job.
- **Formats:** plain text, Markdown, CSV and source code by MIME type; PDF; DOCX, XLSX and
  PPTX (zip archives of XML, read with `zip` and `quick-xml`). Pick the PDF crate by trying
  `pdf-extract` and `lopdf` on a corpus of real files first and record the choice in
  `docs/backend.md`. Images are not OCR'd; that is a different feature with a model behind it.
- **Bounds, because these are untrusted files:** skip objects over 25 MB; stop reading at
  1 MB of extracted text; cap a zip's total uncompressed size and entry count before
  inflating (zip bombs); run each extraction in `spawn_blocking` under a timeout with
  `catch_unwind`, because PDF parsers panic on malformed input. A failure is a `failed`
  row, never a crashed worker.
- Retention, erasure and channel deletion already delete the `files` row; the cascade takes
  the text with it. The export does not include extracted text: it is derived data.

### 2. Search

- The files route from CS-063 matches the name **or** the content
  (`search_vector @@ plainto_tsquery(search_text_config(), search_normalize($q))`), ranking a
  name match above a content match.
- A content match returns a snippet (`ts_headline` with `StartSel`/`StopSel` markers the
  client renders as highlight, never as HTML).
- The Files tab shows the snippet under the file name.

### 3. Operations

- A `file_text_extraction` metric (done / failed / skipped by reason) and the backlog size on
  the existing metrics endpoint, and a RUNBOOK note: what the backfill costs on an instance
  with N GB of files, and how to watch it.

## Acceptance

- [ ] Text inside PDF, DOCX, XLSX, PPTX and plain-text files is found by search, with a
      highlighted snippet, under the same visibility rules as CS-063.
- [ ] Existing files are backfilled without a manual step; a file that cannot be read is
      recorded once and not retried.
- [ ] A zip bomb, a 200 MB PDF, a malformed PDF and a password-protected PDF each end as a
      status row, and the worker keeps running.
- [ ] Deleting a file, or a retention purge, removes its text.

## Tests

Extractor unit tests over a fixtures directory: one file per format, an empty PDF, a
malformed PDF, an encrypted PDF, a zip bomb, an oversized file. `http_tests` for a content
match, a name match ranked above it, the snippet, and visibility. A worker test that two
claimers never extract the same file twice.
