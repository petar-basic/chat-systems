# CS-053 — Rich previews from connected apps (Jira, GitHub, Google Drive)

**Wave:** 15 — links people actually paste at work
**Area:** backend · frontend · ops
**Blocked by:** ~~CS-049~~ ✅ shipped (the preview pipeline this extends)
**Blocks:** —

## Problem

CS-049 previews a link by fetching the page anonymously and reading its OpenGraph tags. That
is right for the public web and useless for the links a team pastes most: a Jira ticket, a
private pull request, a Google Doc. Behind a login those pages answer with an app shell, so
since 2.1.1 they get no card at all. Slack shows a real card for them — type, status,
assignee, an *Open* button — because it does not read the page: an installed app asks the
service's API about that one object, with someone's permission.

## What "straightforward for an admin" means here

A self-hosted instance cannot ship OAuth credentials: Atlassian, GitHub and Google bind an
OAuth app to the redirect URL of the server that uses it. So an admin registers one app per
service, once, and the product makes that a two-minute, copy-and-paste job:

1. **Workspace settings → Apps** lists Jira Cloud, GitHub and Google Drive, each with
   *Set up*.
2. *Set up* shows the exact values to paste into the service's developer console — the
   redirect URL, the scopes, the app name and logo — each with a copy button, and a link
   straight to the page where the app is created.
3. The admin pastes back the client ID and secret, presses *Test*, and the app is on for
   the workspace.
4. Each person then sees *Connect Jira* (or GitHub, or Google) the first time they paste a
   link the app could preview, and once in their profile. One click, the service's own
   consent screen, done.

RUNBOOK gets a page per service with screenshots of the console steps; the product shows the
same steps inline, so nobody has to leave the setup screen to follow them.

## Approach

### 1. One integration framework, three providers

- **`integrations` feature.** `workspace_apps (workspace_id, provider, client_id,
  client_secret_encrypted, enabled, configured_by, configured_at)` and `app_connections
  (user_id, workspace_id, provider, external_account, access_token_encrypted,
  refresh_token_encrypted, expires_at, scopes, connected_at)`. Secrets and tokens are
  encrypted with ChaCha20-Poly1305 the way TOTP secrets already are (`auth/totp.rs`), under
  a key derived from `JWT_SECRET` with its own domain separator — so the database dump that
  leaks a token is the one that already must not leak.
- **OAuth 2.0 authorization code with PKCE**, `state` bound to the user's session, refresh
  handled by the worker before expiry, a disconnect button, and revocation at the provider
  on disconnect where the provider supports it. Deactivating a user (including by SCIM)
  deletes their connections.
- **A provider trait**: `matches(url) -> Option<ObjectRef>` and `fetch(ref, token) ->
  AppCard`. The unfurl consumer from CS-049 asks the providers first, in order, and falls
  back to OpenGraph only when none claims the URL. Provider hosts are fixed per provider,
  so these requests do not go through the SSRF guard's resolve-anything path.

### 2. Whose permission

**The poster's.** A card is fetched with the connected account of whoever posted the link,
which is what Slack does and the only model that cannot leak: everyone in the channel sees
what the poster could already see and chose to share. A single workspace token would show
any member the title and status of tickets in projects they have no access to.

If the poster has not connected the app, the link stays a plain link and the poster alone
gets a one-line prompt under their message — *Connect Jira to show a preview* — that
disappears once used or dismissed.

A card is a snapshot: it says *as of* its fetch time, and the poster can refresh it.

### 3. The card

`metadata.unfurls[]` gains a second kind next to the OpenGraph one:

```json
{ "kind": "app", "provider": "jira", "url": "…", "title": "Focus Axon planning on four project types",
  "subtitle": "Story PT-262", "icon": "story",
  "fields": [{ "label": "Status", "value": "In Progress", "tone": "info" },
             { "label": "Priority", "value": "Medium" },
             { "label": "Assignee", "value": "Lazar Stojanović", "avatar": "…" }],
  "fetched_at": "…" }
```

One generic renderer for every provider: icon, title, subtitle, a two-column field grid,
*Open in …*, the *as of* line. No provider-specific frontend code.

### 4. Providers

| Provider | Links | API and scope | Card |
|---|---|---|---|
| **Jira Cloud** | `https://{site}.atlassian.net/browse/{KEY-123}` | OAuth 2.0 (3LO), `read:jira-work`, `offline_access`; `GET /ex/jira/{cloudId}/rest/api/3/issue/{key}?fields=summary,status,priority,assignee,issuetype` | type and key, summary, status, priority, assignee |
| **GitHub** | issues, pull requests, commits, repositories on `github.com` | GitHub App with read-only *Issues*, *Pull requests*, *Contents* and *Metadata* permissions (user-to-server tokens) | PR/issue number and title, state (open, merged, closed, draft), author, labels, review and check summary for PRs |
| **Google Drive** | `docs.google.com/{document,spreadsheets,presentation}/d/{id}`, `drive.google.com/file/d/{id}` | OAuth 2.0, `drive.metadata.readonly`; `files.get?fields=name,mimeType,owners,modifiedTime,iconLink` | file type, name, owner, last modified |

Public GitHub objects can be carded without a connection (the REST API answers anonymously,
within its rate limit), so a public PR gets a real card for everyone.

### 5. Order of work

1. ~~Framework and Jira~~ ✅ shipped — see history.md. The provider list, tables, sealing,
   OAuth flow, consumer hook, card renderer, *Apps* panel and callback page are in place.
2. GitHub — a `Provider::Github` variant, its matcher, fetch and card mapping, its setup
   steps; anonymous cards for public repositories.
3. Google Drive — the same, for Docs, Sheets, Slides and Drive files.

## Acceptance

- [ ] An admin can set up each provider from *Workspace settings → Apps* following only the
      on-screen steps, and *Test* says whether the credentials work.
- [ ] A person connects and disconnects their account in one click each; disconnecting
      revokes at the provider where it can and deletes the tokens.
- [ ] A Jira issue, a GitHub issue / pull request / commit, and a Google Doc each get a card
      built from the API, fetched with the poster's account.
- [ ] A poster without a connection gets a private *Connect* prompt and no card; nobody else
      sees anything.
- [ ] A member who cannot open the object in the service learns nothing from the card that
      the poster did not choose to share by posting it.
- [ ] Tokens and client secrets are encrypted at rest; refresh happens before expiry; a
      deactivated user's connections are gone.
- [ ] RUNBOOK has a setup page per provider; the audit log records app setup, enable,
      disable and every connect / disconnect.

## Tests

`http_tests` for app setup and its admin-only authorization, connect / callback / disconnect
(the provider faked by a local server), state and PKCE mismatches refused, and the poster-
permission rule. Provider unit tests for URL matching and card mapping against recorded API
responses. One frontend test for the generic card renderer and one for the *Connect* prompt.
