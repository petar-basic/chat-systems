# CS-055 — Files and images the way Slack does them

**Wave:** 17 — files and images
**Area:** backend · frontend · ops
**Blocked by:** —
**Blocks:** —

## Problem

Files work, but next to Slack every step feels older:

- **Sending.** A file goes out the moment it is picked, pasted or dropped, as a message of
  its own whose body is `[file: name](url)`. There is no tray to look at it first, no text
  alongside it, and a pasted screenshot is posted before you have seen which one it was.
  Five files are five messages. There is no progress and no cancel, only a spinner on the
  paperclip. The composer refuses anything over 25 MB while the server takes 100 MB.
- **Showing.** Every image is loaded at full size: `files.thumbnail_key` exists and is never
  written. Nothing records an image's size, so the virtualized list jumps when one loads.
  Several images are a column of full-height pictures, never a grid. Every non-image is the
  same grey icon with no size, and video or audio cannot be played.
- **Names.** `sanitize_filename` turns everything outside ASCII into `_`:
  *Izveštaj o radu.pdf* is stored and shown as *Izve_taj_o_radu.pdf*, and a name with `..`
  becomes *file* and loses its extension.
- **Privacy.** A phone photo keeps its EXIF block, GPS position included.
- **Viewing and finding.** The lightbox shows one image; there is no next, no zoom, no
  who-and-when. A channel has no *Files* view, though `files` knows what belongs where.
- **Serving.** A download reads the whole object into memory and answers without `Range`,
  so a 100 MB file costs 100 MB of RAM per request and a video could not seek even if we
  played it.

## Approach

Six parts, each its own PR, in this order.

### 1. Attachments belong to the message, not to its text

**Model.** `files.message_id` already links a file to its message (today by parsing the
storage key out of the content). It becomes the only link:

- `Message` gains `attachments: Attachment[]` — `id, filename, mime_type, size_bytes, url`,
  and from part 2 `width, height, thumbnail_url` — batch-loaded with the page like reactions.
- `SendMessageRequest` and scheduled messages gain `attachment_ids` (at most 10). Each must
  be a draft file the sender uploaded to this workspace; they are linked in the message's
  own transaction, so a message never appears without its files. `content` may be empty
  when there are attachments.
- `UpdateMessageRequest` gains `attachment_ids` as the set to **keep**: an edit can remove a
  file (which deletes it, as today) but not add one, as in Slack.
- **Drafts.** Composer uploads are stored with `files.draft_until = now() + 24h`; linking
  clears it, and `chat-worker` deletes drafts past their time. Avatars and custom emoji keep
  uploading as today and are never drafts.
- **Forwarding** a message with files copies them server-side into the new message
  (`copy_attachments_from`, readable by the forwarder only), so deleting either message
  still deletes exactly its own files.
- Push, mention e-mails and the sidebar preview say *sent a file* / *sent 3 files* when the
  text is empty.

**Migration.** Every `[file: …](/api/files/download/…)` in message bodies is linked by key
(where not already) and removed from the text; a message that was only that link ends with
empty content. `extract_file_keys` and the content-parsing link/release paths go away, and
the Slack importer writes attachments the new way.

**Composer.** Picking, pasting or dropping files puts them in a tray above the input — image
thumbnails, a type icon and size for the rest, a ✕ on each — and uploads them right away
with a progress bar per file (XHR, so it can be cancelled). Send posts the text and the
files together; Send is disabled while anything is still uploading. The size limit comes
from the server (`max_upload_bytes` in the instance config) instead of a constant.

**Message.** Text first, then the files. One image is shown at its natural aspect up to
360 px; two to four make a grid; more show four with *+N*. Other files are cards.

### 2. Thumbnails, sizes and EXIF

Images (PNG, JPEG, GIF, WebP; sniffed from the bytes, not trusted from the client's
`Content-Type`) are buffered at upload — capped at 30 MB and 50 megapixels, so a
decompression bomb is refused before it is decoded — and processed on a blocking thread:

- **Orientation** from EXIF is applied, then **width and height** are stored.
- **A thumbnail** at most 720 px wide (crisp at 360 px on a 2× screen) is stored next to the
  original as WebP, under `thumbnail_key`.
- **The original's metadata is stripped** — EXIF, XMP and comments in JPEG; `eXIf`/text
  chunks in PNG; `EXIF`/`XMP` in WebP — without re-encoding the pixels. A JPEG whose
  orientation was not upright gets a minimal EXIF block holding only that orientation, so
  it still displays the right way up. GPS never reaches storage.
- An animated GIF keeps its animation in the original; the thumbnail is its first frame,
  and the message shows the original.

The message reserves the image's box from `width`/`height` before it loads, so the list
stops jumping. Downloads of thumbnails and originals get `Cache-Control: private,
max-age=31536000, immutable` — storage keys never change content.

### 3. File names as people wrote them

`files.filename` keeps the name as uploaded — Unicode (NFC), spaces and all — with path
separators, control characters and leading dots removed and at most 200 characters. The
storage key keeps today's ASCII-safe form. `Content-Disposition` sends both
`filename="ascii fallback"` and `filename*=UTF-8''…` (RFC 6266/5987). Names stored before
this part stay as they were; the original was never kept.

### 4. Lightbox and file cards

- **Lightbox as a gallery**: ← / → and swipe through the conversation's images, newest
  last; zoom by click or pinch; who posted it and when; *Open original* and *Download*;
  Esc closes.
- **Cards by type**: PDF, document, spreadsheet, presentation, archive, code / text, audio,
  video and generic icons, each with its size and type; images that failed to render fall
  back to a card.

### 5. Video and audio

- Downloads **stream** from storage (`object_store` ranged reads) and answer `Range` with
  `206 Partial Content`, `Accept-Ranges` and `Content-Length`; memory per download stops
  depending on the file's size.
- MP4, WebM and MOV play in the message (`<video controls preload="metadata">`, 16:9 box);
  MP3, M4A, OGG, WAV and WebM audio get an `<audio>` player. Anything the browser cannot
  play stays a card.

### 6. A channel's files

`GET /channels/{channel_id}/files?kind=image|video|audio|document&cursor=` lists the files
of a channel's live messages, newest first, behind the same access check as reading the
channel (guests included). The channel header gains *Files*: a panel with a grid of image
thumbnails and a list for the rest, filterable by kind, each opening the lightbox or the
message it belongs to.

## Deliberately not here

- **HEIC to JPEG conversion.** It needs libheif, a C dependency in the image; HEIC stays a
  file card until someone asks.
- **Video thumbnails and transcoding.** ffmpeg in the image is out of proportion for this.
- **Editing alt text, image annotation, Slack's clips (recording audio or video in the
  composer).** Each is a ticket of its own.

## Acceptance

- [ ] Files picked, pasted or dropped wait in the composer with progress, can be removed,
      and go out with the text as one message; a pasted screenshot is never sent unseen.
- [ ] Up to 10 files in a message; images show as one picture or a grid with *+N*.
- [ ] Existing file messages show as before after the migration, now as attachments.
- [ ] Editing can remove a file; forwarding carries the files; deleting a message deletes
      its files; abandoned drafts are gone within a day.
- [ ] Images load as thumbnails with their box reserved — no jump — and the original opens
      in the lightbox; GPS is not in any stored original.
- [ ] *Izveštaj o radu.pdf* is stored, shown and downloaded under that name.
- [ ] The lightbox steps through the conversation's images; non-image cards say type and
      size.
- [ ] A video seeks; a download's memory does not grow with the file.
- [ ] A channel's *Files* panel lists what was shared there, by kind, to whoever can read
      the channel.

## Tests

`http_tests` for send with attachments (only your own drafts, at most 10, same workspace,
empty content allowed only with files), edit keeping a subset, forward copying, draft
expiry, the migration on seeded legacy messages, ranged downloads (`206`, `Content-Range`,
invalid ranges `416`), the channel files list's access matrix (member / guest / outsider /
no token). Unit tests for image sniffing, the size caps, orientation, EXIF stripping (a
fixture with GPS comes out without it and keeps its orientation), thumbnail dimensions,
filename normalisation and the `Content-Disposition` encoding. Frontend tests for the
composer tray (add, progress, remove, send together, paste does not send), the grid with
*+N*, reserved image boxes, the gallery's keyboard navigation and the type cards.
