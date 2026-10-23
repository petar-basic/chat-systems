# CS-061 — Keyword notifications

**Wave:** 21 — notifications that fit how a team works
**Area:** backend · frontend
**Blocked by:** CS-060
**Blocks:** —

## Problem

Some words matter to a person more than any channel does: their product's name, "deploy",
"outage", a customer's name. Slack lets you list them under "My keywords" and notifies you
wherever one is written, in any channel you are in, as if you had been mentioned. Here the
only way to hear about a word is to sit in every channel on `all`, which CS-060 makes
possible and nobody wants.

## Approach

### 1. The list

- `notification_keywords (user_id, workspace_id, keyword, normalized, created_at)`, unique on
  `(user_id, workspace_id, normalized)`, at most 50 per person per workspace.
- `normalized` comes from the same `search_normalize` the search index uses (lowercased,
  diacritics folded). So `Čačak` matches `cacak` here exactly as it does in search, and
  there is one definition of "the same word".
- A keyword is a word or a short phrase, 2–64 characters, no leading `@` or `#` (those
  are mentions and channels).
- `GET` / `PUT /workspaces/{id}/notification-keywords` replaces the whole list. It is a
  settings field, not a collection people page through.

### 2. Matching

- The notification consumer matches after CS-060 resolves levels, in **one SQL query per
  message**: channel members whose keywords appear in the normalized text as whole words
  (`\m…\M` with the keyword escaped for the regex). The work stays in the database next
  to `search_normalize`, not in a second Rust normalizer that would drift from it.
- Who is skipped: the author; anybody already notified for this message as a mention or
  DM (one notification per message per person); channels at `nothing` or muted; people
  in DND, for push only, as with mentions.
- A match produces `NotificationType::Keyword` (new enum value): an Activity row with
  "matched *deploy*", a push when offline, and the `notification` frame with
  `kind: "keyword"`. No email; Slack does not email keywords either, and the mention digest
  is already the email people tolerate.
- **The mention badge does not move.** The unread and mention counters are bumped inside
  the transaction that writes the message (CS-050); matching keywords there would put a
  per-member regex scan on the send path. The Activity panel's Unread filter is where a
  keyword shows up. This is written down in `docs/frontend.md` so nobody files it as a bug.

### 3. The UI

- **Preferences → Notifications**: "My keywords", an input that adds chips, saved with the
  rest of the section.
- **In the message list**, a matched keyword is highlighted with the mention background but
  without the mention pill. The highlighter in `MessageContent` already takes a label set for
  mentions (see [frontend.md](../frontend.md)); keywords join it client-side from the same
  normalized list.

## Acceptance

- [ ] A message containing a listed keyword, in a channel the person is in, produces one
      Activity row and one push, and none for the author.
- [ ] Matching ignores case and diacritics and respects word boundaries: `deploy` matches
      "Deploy!" and "deploý", not "redeployed".
- [ ] A message that both mentions and matches produces one notification, the mention.
- [ ] Muted and `nothing` channels never notify for keywords.
- [ ] The keyword is highlighted in the message for the person who listed it, and for
      nobody else.

## Tests

`http_tests` for the list (ok, limit, duplicate after normalization, too short, the
authorization matrix). Consumer tests: whole-word matching, diacritics, regex
metacharacters in a keyword (`c++`, `a.b`), dedup against mentions, muted and `nothing`,
and a private channel the person is not in. Frontend: the chips editor and the highlight.
