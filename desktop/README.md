# Chat Systems desktop

A Tauri 2 app for macOS and Windows that opens any Chat Systems server. It does not bundle
the web client: each server is a webview in one window, showing that server's own web app,
so the app always matches the version of the server it talks to. Why it is built this way is
in [docs/native-apps-plan.md](../docs/native-apps-plan.md).

## Layout

| Path | What it is |
|---|---|
| `ui/` | The app's own pages: the server rail (`rail.html`) and *Add a server* (`index.html`) |
| `src-tauri/src/lib.rs` | The window, its webviews, the menu and the commands the app's pages call |
| `src-tauri/src/servers.rs` | Normalising an address, checking it is a Chat Systems server, and the saved list |
| `src-tauri/src/navigation.rs` | Which links stay in the app, and where downloads go |
| `src-tauri/src/bridge.js` | `window.chatSystemsDesktop`, injected into each server's page: how the page reports its workspaces, raises a notification and hears the rail's and the notifications' clicks |
| `src-tauri/src/workspaces.rs` | What the app accepts from a page's report: ids, names, unread counts, icons from that server only |
| `src-tauri/src/notifications.rs` | What the app accepts from a page's notification: a key it can hand back, a title and a body cut to banner length |
| `src-tauri/capabilities/` | Which pages may call the app: the rail and *Add a server*; a server's page gets only `report_workspaces` and `notify`, granted at run time for its own origin |

## Run and build

Needs Rust 1.98 and Node 24.

```bash
npm install
npm run dev
npm run build
```

`npm run build` writes the app to `src-tauri/target/release/bundle/`. Start the built app
from Finder or with `open`: macOS asks for the camera and microphone on behalf of whatever
launched the app, and refuses at once when that is a terminal. Notifications need the built
app as well: macOS shows them only for a signed bundle, so `npm run dev` raises none. The
build is signed ad hoc (`signingIdentity: "-"`), which turns on the hardened runtime, and
`src-tauri/Entitlements.plist` gives the camera and microphone back to huddles.

Every server's page is loaded at start and keeps running while it is hidden
(`background_throttling` is off), so a server you are not looking at still raises
notifications and keeps its unread count on the rail and the Dock.

Closing the window hides it and the app keeps running, so notifications still arrive; the
tray menu opens it again, turns *Open at Login* on or off, and quits. Opened at login, the
app starts with its window hidden.

The servers you add are kept in `servers.json` in the app's config directory
(`~/Library/Application Support/com.chatsystems.desktop` on macOS).
