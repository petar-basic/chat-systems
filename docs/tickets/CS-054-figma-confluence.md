# CS-054 — Slack-grade app previews, Figma and Confluence

**Wave:** 16 — the rest of what a team pastes
**Area:** backend · frontend · ops
**Blocked by:** ~~CS-053~~ ✅ shipped (the connected-apps framework this extends)
**Blocks:** —

## Problem

CS-053 gave Jira, GitHub and Google Drive links a real card, fetched with the poster's
account. Put next to Slack, three things are still visibly missing, and two services a
design-and-docs team pastes all day are not covered at all:

1. **The prompt has no "Never ask me again".** Slack's ephemeral *Connect* message offers
   *Not now* and *Never ask me again*. Ours only has the per-message ✕, so someone who will
   never connect Jira is asked under every ticket link they post.
2. **Cards have no picture.** A Figma or Slack card for a design shows its thumbnail; the
   design is the point. `Unfurl.image_url` exists, but the app card ignores it.
3. **Apps is a list of forms, not a directory.** Slack's *Apps* shows each app with its
   icon, whether you are connected, and one line on what it does; the same icon heads every
   card the app posts. Ours shows names and status words.
4. **A video link is a still picture.** Slack plays YouTube, Vimeo and Loom inline. We show
   an OpenGraph card, and from a server in the EU YouTube often answers with its consent
   page, so not even that.
5. **Figma and Confluence links get nothing**, because both sit behind a login.

## What stays different from Slack, and why

Slack's apps are built and hosted by the vendor, so an admin installs one with a click. A
self-hosted instance cannot ship OAuth credentials — the vendors bind them to the redirect
URL — so an admin still registers one app per service, once, from the on-screen steps
CS-053 introduced. A central broker would remove that step by routing every customer's
tokens through one server, which defeats self-hosting. Everything a member sees is meant to
match Slack.

## Approach

Four parts, each its own PR, in this order.

### 1. Slack's prompt, picture and directory

- **Never ask me again.** `app_prompt_optouts (workspace_id, user_id, provider,
  created_at)`, keyed on the membership like `app_connections` so a member who leaves
  takes their opt-outs with them. `PUT`/`DELETE /workspaces/{ws_id}/apps/{provider}/prompts`
  for the member themself. The unfurl consumer treats an opted-out poster as having no app
  for that provider: no prompt, and the link falls through to the OpenGraph preview.
  Connecting deletes the opt-out. The prompt's ✕ becomes *Not now* (this message only, as
  today) beside *Never ask me again*.
- **Picture.** The app card shows `image_url` as a wide thumbnail under the fields, only for
  `https://` URLs, with `referrerPolicy="no-referrer"`, lazy, and removed if it fails to load
  — provider thumbnail URLs are signed and expire, and the card's refresh button fetches a
  new one. Copying thumbnails into our own storage is not worth it for a snapshot.
- **Directory.** *Apps* lists every provider with its brand icon, one line on what it
  previews, and its state for you — *Connected as …*, *Connect*, *Not set up* — with setup
  folded under each for admins, as now. A member who chose *Never ask me again* sees
  *Prompts off · Turn on*. The same icon heads the app card and the prompt. Icons come from
  `simple-icons` (CC0), imported one by one.

### 2. Video links play inline

YouTube, Vimeo and Loom are recognised by URL and described through their **oEmbed
endpoints** — fixed hosts, no discovery, so the SSRF guard's resolve-anything path is not
involved — which answer without a consent wall: title, author, provider, thumbnail. The
card shows the thumbnail with a play button; pressing it swaps in an `<iframe>` whose `src`
the client builds from the video ID (`youtube-nocookie.com/embed/{id}`,
`player.vimeo.com/video/{id}`, `loom.com/embed/{id}`). The oEmbed `html` field is never
rendered. `frame-src` in the CSP gets exactly those three hosts. `Unfurl` gains
`kind: "video"` and `embed: { provider, id }`.

### 3. Figma

| | |
|---|---|
| **Links** | `https://www.figma.com/{design,file,proto,board,slides,deck,site,make}/{key}/…`; `…/{key}/branch/{branch_key}/…` previews the branch |
| **OAuth** | `https://www.figma.com/oauth` with PKCE (S256); token `POST https://api.figma.com/v1/oauth/token` and refresh `POST https://api.figma.com/v1/oauth/refresh`, both form-encoded with the client ID and secret as HTTP Basic auth. The refresh answer carries no new refresh token, so the stored one is kept (`store_refreshed_tokens` already coalesces) |
| **Scopes** | `file_metadata:read`, `current_user:read` — the narrowest pair that names the file and the account |
| **Call** | `GET /v1/files/{key}/meta` → `name`, `folder_name`, `editorType`, `last_touched_at`, `last_touched_by`, `thumbnail_url` |
| **Card** | name; *Figma design* / *FigJam board* / *Figma Slides* / … from `editorType`; *Project* (`folder_name`); *Last edited* with who; the thumbnail |
| **Setup** | figma.com/developers/apps → create the app inside the organisation's team and publish it as **Private**: no Figma review, and only that organisation's people can authorise it, which is what a self-hosted workspace wants |

Endpoints are configurable (`FIGMA_WEB_URL`, `FIGMA_API_URL`) only so tests can point them
at a fake server, like the other providers.

### 4. Confluence

Slack has separate *Jira Cloud* and *Confluence Cloud* apps, and so do we: `confluence` is
its own provider with its own connection and consent, so connecting one never widens what
the other can read.

| | |
|---|---|
| **Links** | `https://{site}.atlassian.net/wiki/spaces/{KEY}/pages/{id}[/…]`, `…/pages/edit-v2/{id}`, `…/wiki/pages/viewpage.action?pageId={id}`, and blog posts `…/wiki/spaces/{KEY}/blog/{yyyy}/{mm}/{dd}/{id}/…` |
| **OAuth** | the Atlassian 3LO flow Jira already uses; sites from `accessible-resources`, matched on host |
| **Scopes** | `read:page:confluence`, `read:space:confluence`, `read:user:confluence`, `read:me`, `offline_access` (granular — Confluence's v2 API refuses the classic ones) |
| **Calls** | `GET /ex/confluence/{cloudId}/wiki/api/v2/pages/{id}` (or `/blogposts/{id}`), `GET …/spaces/{spaceId}`, `POST …/users-bulk` for the owner's and last editor's names, in parallel |
| **Card** | title; *Page* or *Blog post* in *{space name}*; *Owner*; *Last updated* with who |

**Setup reuses the Jira app.** One Atlassian app can carry both products, so when Jira is
already set up, Confluence's setup offers *Use the Jira app*: the admin adds the Confluence
API and its scopes to that app in Atlassian's console, and our side copies the client ID and
the sealed secret server-side (`ConfigureAppRequest.reuse_from: "jira"`, accepted only for
that pair). The secret never travels back to the browser.

Parts 3 and 4 each widen the `workspace_apps.provider` check in a migration of their own.

## Acceptance

- [ ] *Never ask me again* stops a member's prompts for that app in that workspace, the link
      gets its OpenGraph preview instead, *Apps* shows it and turns it back on, and
      connecting clears it.
- [ ] An app card with a picture shows it, and a picture that fails to load leaves no gap.
- [ ] *Apps* shows each app's icon, what it previews, and the member's state in one glance;
      cards and prompts carry the same icon.
- [ ] A YouTube, Vimeo or Loom link shows its thumbnail and plays inline on click; the iframe
      `src` is built by the client from the ID and the CSP allows only those three hosts.
- [ ] A Figma design, FigJam board and Slides deck each get a card with a thumbnail, fetched
      with the poster's Figma account; a file they cannot open gets no card.
- [ ] A Confluence page and blog post each get a card with space, owner and last edit,
      fetched with the poster's Confluence connection; a Jira connection alone does not.
- [ ] Confluence can be set up from the Jira app's credentials without pasting the secret.
- [ ] RUNBOOK has a setup section for Figma and Confluence; the audit log records them like
      the other apps.

## Tests

`http_tests` for the opt-out (member-only, consumer skips the prompt, connect clears it),
Figma and Confluence against fake servers (connect, card, refresh keeping the refresh
token, not-visible → no card), `reuse_from` admin-only and refused for any pair but
Jira → Confluence. Unit tests for the Figma, Confluence and video URL matchers and the
card mappings. Frontend tests for the prompt's two buttons, the thumbnail's failure path
and the click-to-play swap.
