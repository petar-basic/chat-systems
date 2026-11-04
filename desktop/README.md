# Chat Systems desktop

A Tauri 2 app for macOS and Windows that opens any Chat Systems server. It does not bundle
the web client: each server is a webview in one window, showing that server's own web app,
so the app always matches the version of the server it talks to. Why it is built this way is
in [docs/native-apps-plan.md](../docs/native-apps-plan.md).

## Download

Every [release](https://github.com/petar-basic/chat-systems/releases/latest) carries the app:
a `.dmg` for macOS (one universal build for Apple silicon and Intel) and a `-setup.exe`
installer for Windows. Once installed, it updates itself: it checks the latest release at
start and every six hours, downloads a newer version in the background, and asks to restart.
An update is installed only when it is signed with the project's update key and the version
it announces is the version that was signed.

The app is not signed with an Apple Developer ID or a Windows certificate yet, so each
system warns the first time you open it:

- **macOS:** open the app once and dismiss the warning, then *System Settings → Privacy &
  Security → Open Anyway*. Or clear the quarantine flag:
  `xattr -dr com.apple.quarantine "/Applications/Chat Systems.app"`.
- **Windows:** SmartScreen shows *Windows protected your PC*; choose *More info → Run anyway*.

## Layout

| Path | What it is |
|---|---|
| `ui/` | The app's own pages: the workspace rail (`rail.html`) and *Add a server* (`index.html`) |
| `src-tauri/src/lib.rs` | The window, its webviews, the menu and the commands the app's pages call |
| `src-tauri/src/servers.rs` | Normalising an address, checking it is a Chat Systems server, and the saved list |
| `src-tauri/src/navigation.rs` | Which links stay in the app, and where downloads go |
| `src-tauri/src/bridge.js` | `window.chatSystemsDesktop`, injected into each server's page: how the page reports its workspaces, its theme and its workspace's colours, raises a notification, starts SSO and hears the rail's and the notifications' clicks |
| `src-tauri/src/workspaces.rs` | What the app accepts from a page's report: ids, names, unread counts, and an icon only as a small PNG the page drew |
| `src-tauri/src/updates.rs` | Checking for an update, and asking to restart into it |
| `src-tauri/tauri.release.conf.json` | What a release build adds: the updater, its endpoint and public key, and signed update bundles |
| `src-tauri/src/notifications.rs` | What the app accepts from a page's notification: a key it can hand back, a title and a body cut to banner length |
| `src-tauri/capabilities/` | Which pages may call the app: the rail and *Add a server*; a server's page gets only `report_workspaces`, `report_theme`, `report_colours`, `notify` and `start_sso`, granted at run time for its own origin |

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

A build from source does not update itself: the updater is configured only in
`tauri.release.conf.json`, which the release adds with `--config`. CI builds the app for
macOS and Windows on every PR that touches `desktop/`, and
[`desktop-release.yml`](../.github/workflows/desktop-release.yml) builds and attaches both
after each release, signing the update bundles with the `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets.

### Your own build

To publish the app under your own name, change `productName` and `identifier` in
`src-tauri/tauri.conf.json` and make the icons from one square PNG with
`npx tauri icon path/to/icon.png`. Keep the `chat-systems` URL scheme: a server's SSO page
opens it to hand the sign-in back to the app. For updates of your own, generate a key with
`npx tauri signer generate -w ~/.tauri/your-app.key`, put its public key and your releases'
`latest.json` URL in `tauri.release.conf.json`, and store the private key and its password as
the two secrets above.

Every server's page is loaded at start and keeps running while it is hidden
(`background_throttling` is off), so a server you are not looking at still raises
notifications and keeps its unread count on the rail and the Dock.

Closing the window hides it and the app keeps running, so notifications still arrive; the
tray menu opens it again, turns *Open at Login* on or off, and quits. Opened at login, the
app starts with its window hidden.

The rail and the title bar follow the theme of the server in front, which its page reports
whenever it changes; until a page has reported one, and on *Add a server*, they follow the
system. When the workspace in front has a theme of its own, the page reports its colours too
(`report_colours`), and the rail and the title bar take them: the rail's background, lines,
text and badges, and the title bar in the rail's colour (on macOS a transparent title bar over
the window's background, on Windows 11 the caption colour). The app takes `#rrggbb` colours
and nothing else, and goes back to the colour mode when the page reports none.

The rail lists workspaces, not servers: every workspace you are in on every server, with
the server's name in its tooltip. A server shows as a tile of its own only while it has no
workspace on the rail, such as before you sign in. A workspace with an icon shows it on the
theme's surface; one without shows its initials on a colour of its own. *+* at the bottom creates a workspace on
one of your servers or adds a server. ⌘1 to ⌘9 (Ctrl on Windows) open the first nine entries on
the rail, which the *Workspaces* menu lists too.

Right-clicking the rail hides a workspace from it, shows the hidden ones again, leaves a
workspace, creates a workspace on that server, or removes the server after asking. A hidden workspace stays hidden in `servers.json`;
it no longer counts toward the Dock badge or raises notifications, and you stay a member of it.
*Leave Workspace…* opens the workspace and asks there, in the page, which knows whether you are
allowed to leave and is the one signed in to do it.

*Sign in with SSO* opens the system browser, since providers such as Google refuse to sign in
inside an embedded web view. The app starts the sign-in with a random nonce; the server's
callback hands the browser a code that lives a minute, and its `/sso/desktop` page opens
`chat-systems://sso?code=…&nonce=…`. The app accepts that link only for a nonce it issued in
the last ten minutes, once, and loads `/sso/complete` in that server's window, which trades the
code for the session cookies there.

The servers you add are kept in `servers.json` in the app's config directory
(`~/Library/Application Support/com.chatsystems.desktop` on macOS).
