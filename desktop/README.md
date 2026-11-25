# Chat Systems desktop

A Tauri 2 app for macOS and Windows that opens any Chat Systems server. It does not bundle
the web client: each server is a webview in one window, showing that server's own web app,
so the app always matches the version of the server it talks to. Why it is built this way is
in [docs/native-apps-plan.md](../docs/native-apps-plan.md).

The same project builds the phone apps for iOS and Android, which work the other way: they
carry a copy of the web client. See [Phones](#phones).

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
| `src-tauri/src/desktop/mod.rs` | The window, its webviews, the menu and the commands the app's pages call |
| `src-tauri/src/desktop/servers.rs` | Normalising an address, checking it is a Chat Systems server, and the saved list |
| `src-tauri/src/desktop/navigation.rs` | Which links stay in the app, and where downloads go |
| `src-tauri/src/desktop/bridge.js` | `window.chatSystemsDesktop`, injected into each server's page: how the page reports its workspaces, its theme and its workspace's colours, raises a notification, starts SSO or a passkey sign-in and hears the rail's and the notifications' clicks |
| `src-tauri/src/desktop/workspaces.rs` | What the app accepts from a page's report: ids, names, unread counts, and an icon only as a small PNG the page drew |
| `src-tauri/src/desktop/updates.rs` | Checking for an update, and asking to restart into it |
| `src-tauri/tauri.release.conf.json` | What a release build adds: the updater, its endpoint and public key, and signed update bundles |
| `src-tauri/src/desktop/notifications.rs` | What the app accepts from a page's notification: a key it can hand back, a title and a body cut to banner length |
| `src-tauri/capabilities/` | Which pages may call the app: the rail and *Add a server*; a server's page gets only `report_workspaces`, `report_theme`, `report_colours`, `report_creation`, `report_other_servers`, `open_link`, `notify`, `start_sso` and `start_passkey`, granted at run time for its own origin |

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

Sharing a whole screen in a huddle opens a see-through window over that display
(`ui/overlay.html`), and the others' strokes are drawn on it, so you see them on the real
screen as well as in the huddle window:
- The window ignores the mouse (`set_ignore_cursor_events`), so clicks go through to whatever
  is under it. Tauri does this on macOS and on Windows alike, so neither falls back to the
  huddle window alone.
- It stays on top and on every Space, and it is kept out of screen captures
  (`content_protected`), so the viewers do not get each stroke a second time inside the video.
- The display is the monitor whose size in pixels matches the shared video, or else the one the
  app's window is on.
- Only the server page that opened the window may draw on it or close it. It takes `#rrggbb`
  colours and points from 0 to 1, nothing else.
- It closes when the share ends or that page is reloaded or left.

A shared window or tab shows the strokes only in the huddle window.

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

*Sign in with a passkey* takes the same way back. A webview is not trusted with an arbitrary
server's passkeys, so the app opens `/passkey/desktop?nonce=…` in the system browser; that page
runs the passkey, gets the same one-minute code from the server and hands it to the app through
`/sso/desktop`.

The servers you add are kept in `servers.json` in the app's config directory
(`~/Library/Application Support/com.chatsystems.desktop` on macOS).

## Phones

The iOS and Android apps are targets of this project (Wave 37, CS-144). A store app that
only loads a remote page is likely to be rejected, so a phone app carries its own build of
the web client (`frontend/dist`) instead of a server's page. The client then talks to any
server across origins with tokens and no cookies, as CS-143 made it and the server do: the
app's origin is `tauri://localhost` on iOS and `http://tauri.localhost` on Android, which the
server allows through `APP_ORIGINS`.

| Path | What it is |
|---|---|
| `src-tauri/src/phone.rs` | The phone app: one window with the web client, a link that leaves it opened in the system browser, and on iOS the web view kept below the status bar |
| `src-tauri/src/phone/session.rs` | Each server's refresh token, in the iOS Keychain (the data-protection keychain, readable while the phone is unlocked) or in shared preferences encrypted with an Android Keystore key: `load_session`, `save_session` and `forget_session`, keyed by the server's URL, through `keyring-core` and its Apple and Android stores |
| `src-tauri/capabilities/phone.json` | On a phone, the app's own copy of the client may open a link in the browser, the mail app or the phone app, keep a server's session, open the sign-in sheet, hear the app's links, show a notification and hear it tapped, and on Android hold a huddle in the background; it calls nothing else |
| `src-tauri/plugins/huddle` | `tauri-plugin-huddle`, the app's own: on Android, `begin` starts a foreground service of the microphone type with an ongoing *In a huddle* notification and `end` stops it, so the microphone stays open with the screen locked |
| `src-tauri/tauri.ios.conf.json`, `tauri.android.conf.json` | What a phone build changes: the identifier `com.chatsystems.mobile`, the web client as the frontend, built first, and a content security policy that lets it reach any server |
| `src-tauri/Info.ios.plist` | Merged into the iOS app's `Info.plist`: why it asks for the camera and the microphone, and the `audio` background mode that keeps a huddle going with the screen locked |
| `src-tauri/gen/apple`, `src-tauri/gen/android` | The Xcode and Android Studio projects, generated by `tauri ios init` and `tauri android init` and committed with the changes below |
| `src-tauri/gen/android/.../MainActivity.kt` | Keeps the web view clear of the status bar, the navigation bar and the keyboard: Android 15 and later draw an app under them |
| `src-tauri/icons/phone.svg` | The phone icon, a full square that iOS and Android round themselves; `npx tauri icon src-tauri/icons/phone.svg --ios-color '#7c3aed'` writes it into both projects (then restore `icons/`, which holds the desktop icons) |

*Sign in with SSO* and *Sign in with a passkey* take the desktop app's way back, through a
sign-in sheet over the app instead of the system browser: `ASWebAuthenticationSession` on iOS
and a Custom Tab on Android, through `tauri-plugin-auth-session`. The sheet opens the same
`/auth/oidc/start?client=desktop` or `/passkey/desktop` page with a nonce and closes when the
server's `/sso/desktop` page opens `chat-systems://sso?code=…&nonce=…`. The client checks the
nonce and trades the code at `/auth/oidc/exchange` for the session. On Android that link goes
to the plugin's `AuthSessionActivity` through the manifest's `.SsoCallback` alias, which holds
the filter for the host `sso`. The deep-link plugin's build script copies its own filter into
every `<activity>…</activity>` in the manifest, so `AuthSessionActivity` is declared there
without a body, which the script skips, and the alias carries its filter.

`chat-systems://open?url=<a link to a server>` opens the app at that link: a server, a channel
or a message on a server it is signed in to, or *Add a server* with that server filled in. The
deep-link plugin registers the scheme for the host `open` (`plugins.deep-link.mobile` in
`tauri.conf.json`), so on Android neither link asks which app should open it. On iOS a link
that launches the closed app is lost until Tauri takes tao 0.38 (CS-146); with the app running
or in the background it arrives.

A message that notifies, while the app is open or in the background, goes through
`tauri-plugin-notification`, which asks the phone the first time. Tapping it opens the message
on iOS; on Android it only brings the app forward, because the plugin's Android side drops the
notification's data and the tap arrives without its id (tauri-apps/plugins-workspace#3550).
With the app closed nothing arrives until the push relay (CS-145). The Android manifest
declares the microphone, the camera and the audio settings, which the web view asks for when a
huddle or a clip starts; the web view cannot share the screen, so a huddle there has no
*Share screen*.

A huddle keeps its microphone with the screen locked: the `audio` background mode on iOS, the
huddle plugin's foreground service on Android. The Android web view still freezes a page that
has been hidden for about five minutes with nothing playing, so in a huddle where nobody makes
a sound, the page cannot reconnect when its sign-in token runs out. The phone then drops out of
the huddle until the screen comes on, when it rejoins within seconds.

Needs, on a Mac, Xcode with an iOS simulator, the Android SDK with an NDK and JDK 17, and
Rust with the targets `aarch64-apple-ios`, `aarch64-apple-ios-sim` and
`aarch64-linux-android`. Each build builds the web client first.

```bash
npx tauri ios build --debug --target aarch64-sim
xcrun simctl install booted "src-tauri/gen/apple/build/arm64-sim/Chat Systems.app"
xcrun simctl launch booted com.chatsystems.mobile
```

```bash
npx tauri android build --debug --target aarch64 --apk
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
adb shell am start -n com.chatsystems.mobile/.MainActivity
```

On an iPhone, a free Apple ID signs the app for seven days. Set `APPLE_DEVELOPMENT_TEAM` to
the team ID Xcode shows for it and build with `--target aarch64`. The first time, run
`xcodebuild -project src-tauri/gen/apple/chat-systems-desktop.xcodeproj -scheme
chat-systems-desktop_iOS -destination id=<the phone's UDID> -allowProvisioningUpdates
DEVELOPMENT_TEAM=<team id>` once: `tauri ios build` builds for a generic device, so without it
the phone is never registered with the team.

Release builds for TestFlight and Play wait on the signing accounts (CS-146).

