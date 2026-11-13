# CS-129 — GIFs

**Wave:** 34 — writing and finding (2026-11-13)
**Area:** backend · frontend · ops
**Blocked by:** —
**Blocks:** —

## Problem

There is no way to send a GIF short of saving one and uploading it. Teams that move from
Slack use `/giphy` daily, and a missing `/giphy` is the first thing they mention.

## Approach

- **The provider is GIPHY**, behind one `GifProvider` trait so another can be added.
  - The instance admin enters an API key in *Admin → Settings*. It is sealed with
    `SECRETS_KEYS` like other stored secrets. Without a key, GIFs are off and nothing shows.
  - A workspace admin can turn GIFs off and pick the rating: G, PG (the default), PG-13 or R.
- **Search goes through the API,** on `GET /workspaces/{ws_id}/gifs?q=` and a trending
  route. The key never reaches the browser, and GIPHY sees the server, not the person. The
  routes are rate limited per person.
- **Read GIPHY's API terms first.**
  - If they allow keeping the file, the chosen GIF is copied into object storage as an
    attachment. A viewer then never contacts GIPHY, and retention, erase and exports treat
    it like any file.
  - If they do not, the message hotlinks GIPHY's media. The CSP gains its media host, and
    the admin setting says that viewers' browsers will fetch from GIPHY.
  - Either way the picker shows the attribution GIPHY requires.
- **In the composer:**
  - A GIF button next to emoji opens a picker with search and trending. Picking a GIF sends
    it with the text typed so far as its message.
  - `/giphy cats` opens the same picker searched for `cats`, so you see the GIF before the
    channel does.
- **In the message:** a GIF plays muted and looping. It pauses when *Reduce motion* is set
  or it scrolls off screen, and the static frame shows until it is hovered. Its title is
  the alt text.

## Acceptance

- [ ] With a key set, `/giphy cats` opens the picker with cats. Picking one posts it, and a
  second browser sees it play.
- [ ] Without a key there is no button, `/giphy` answers that GIFs are off, and the routes
  answer `404`.
- [ ] A workspace with GIFs off has no button and refuses the routes.
- [ ] Results above the workspace's rating never come back.
- [ ] With *Reduce motion* on, GIFs do not animate until hovered.

## Tests

`http_tests` against a fake `GifProvider`:
- search and trending through the routes, with the rating passed through;
- the off switches at instance and workspace level;
- the rate limit;
- the stored-copy path, or the hotlink CSP, whichever the terms allowed.

Frontend: the picker and the motion rules. E2E with the fake provider.
