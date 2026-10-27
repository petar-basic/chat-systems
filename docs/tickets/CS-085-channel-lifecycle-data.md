# CS-085 — Archive, export and retention that say what they did

**Wave:** 26
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

- Archiving a channel does not stop posting to it, publishes no event, and cannot be undone.
- An export without DMs still lists the DM files in `files.jsonl`, and ignores the export's
  date range for files.
- The retention audit row of a workspace counts what the purge removed in every workspace.

## Approach

- Posting, reacting, scheduling and hooks refuse an archived channel (403); archiving and
  *Unarchive* (workspace admins and the channel's admins) publish `channel.updated` and post
  a system message; the composer is replaced by an *Archived* bar.
- `files.jsonl` takes the same DM filter and date range as messages.
- The purge counts per workspace inside its loop; the global total is logged, not audited.

## Acceptance

- [ ] Nothing new appears in an archived channel; unarchive brings it back.
- [ ] An export without DMs contains nothing from a DM.
- [ ] Each workspace's retention audit row is its own count.

## Tests

Each write path on an archived channel; unarchive permissions; export file filter; purge
counts across two workspaces.
