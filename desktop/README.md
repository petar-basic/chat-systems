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
| `src-tauri/src/bridge.js` | `window.chatSystemsDesktop`, injected into each server's page: how the page reports its workspaces and hears the rail's clicks |
| `src-tauri/src/workspaces.rs` | What the app accepts from a page's report: ids, names, unread counts, icons from that server only |
| `src-tauri/capabilities/` | Which pages may call the app: the rail and *Add a server*; a server's page gets only `report_workspaces`, granted at run time for its own origin |

## Run and build

Needs Rust 1.98 and Node 24.

```bash
npm install
npm run dev
npm run build
```

`npm run build` writes the app to `src-tauri/target/release/bundle/`. Start the built app
from Finder or with `open`: macOS asks for the camera and microphone on behalf of whatever
launched the app, and refuses at once when that is a terminal.

The servers you add are kept in `servers.json` in the app's config directory
(`~/Library/Application Support/com.chatsystems.desktop` on macOS).
