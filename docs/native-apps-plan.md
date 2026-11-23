# Native apps

How Chat Systems gets a desktop app for Windows and macOS, and later apps for iOS and Android,
without giving up the point of the project: everyone runs their own server.

Decided 2026-11-02. The desktop work is [Wave 29](./tickets/INDEX.md#wave-29--a-desktop-app);
mobile is a later wave, and its open questions are at the end of this page.

## One app, any server

We build and publish **one** app. On first start it asks for a server address, the way the
web client's sign-in form already does, and it can hold several servers at once. Mattermost,
Element and Rocket.Chat ship the same way.

The other model, where every operator builds, signs and publishes their own app, was
rejected. Each company would need an Apple Developer account, notarisation, a Windows
signing certificate and store listings of its own, and in practice nobody would do it, so
the app would exist only on paper. An operator who wants their own name and icon, or
distribution through MDM, builds from source; that is documented, not the default.

## Desktop: a Tauri 2 shell around the server's own web app

- **One window, a webview per server.** A rail down the left edge lists the workspaces of
  every server (a server shows alone only until it has one); each server is a webview of
  its own in the same window, kept alive while another one is shown, so every server's
  socket stays open.
- **The app does not bundle the frontend.** Each server's webview loads that server's web
  app from its URL. The app is therefore always the same version as the server it talks
  to, whether that server runs 3.7 or 5.0, and no API compatibility promise is needed for
  desktop. It also means sign-in, cookies and the realtime socket are exactly what the
  browser does today: same origin, nothing new on the server.
- **Tauri 2, not Electron.** The project is Rust already, the bundle is around 12 MB instead
  of around 150 MB, and the web view is the system's (WebView2 on Windows, WKWebView on
  macOS). The risk was WKWebView's support for camera, microphone and screen sharing in
  huddles; CS-106 checked it first (below), and it holds.
- **No push relay.** While the app runs, in the tray if its window is closed, it holds the
  realtime socket and raises native notifications itself. Web Push stays for browsers and
  the installed PWA.
- **The remote page gets almost nothing.** The app shows content from servers it does not
  control, so a server's page may call only a handful of native commands (notify, badge,
  tray state, open a link in the browser), only from the origins the person added, and
  nothing that touches files or a shell. Navigation away from a server's origin opens the
  system browser.
- **Updates** come from GitHub Releases through Tauri's updater, verified against our own
  update-signing key. Signing with an Apple Developer ID and a Windows certificate is
  decided separately (see below); until then macOS asks for *right-click → Open* once and
  Windows shows a SmartScreen warning.

## What the spike found (CS-106)

A throwaway Tauri 2.12 app (wry 0.57) on macOS 26.6, one window on the staging server, with
the camera and microphone descriptions in `Info.plist`, tested on 2026-11-02:

| Check | Result |
|---|---|
| Password sign-in, and still signed in after the app restarts | works |
| A huddle with voice and camera, both ways, on LiveKit (staging promotes at once) | works |
| Sharing the screen in a huddle | works |
| The in-app sound for a new message while the app is in the background | works |
| A macOS notification banner | missing: `Notification` exists but never shows one |
| The unread badge on the Dock icon | missing: `navigator.setAppBadge` is undefined |

`getUserMedia`, `getDisplayMedia`, `RTCPeerConnection` and service workers are all present;
`PushManager` is not, which the plan already assumed. Two things worth knowing for CS-107 and
CS-108:

- **macOS asks for the camera on behalf of whoever launched the app.** Started from a
  terminal or another app, the request is refused at once with `NotAllowedError`; started
  as an app (Finder, Dock, `open`), it asks and then works. Test the app the way people start it.
- **Tauri's bridge reaches a remote page** (`__TAURI_INTERNALS__` is there), but calling an
  app command from it needs that command allowed in the capability for those origins.

The banner and the badge are exactly what CS-108 bridges, so nothing changes in the plan.

## What the spike found on phones (CS-142)

The app was a throwaway Tauri 2.12 app (wry 0.57.1), tested on 2026-11-23. It had a local test page and a small plugin, in Kotlin and Swift, with one Rust command. The plugin asked for permissions, registered for push, rang a huddle through CallKit or ConnectionService, and on Android ran a foreground service for the microphone. It ran on:
- an Android 16 emulator;
- an iOS 27.2 simulator;
- an iPhone 13 Pro on iOS 27.2.1, signed with a free Apple ID.

There was no Android phone and no paid Apple Developer account. The audio test sent the microphone, and a beep every 5 s, through a WebRTC connection inside the page, and counted the packets and the sound level each second.

| Check | Result |
|---|---|
| Microphone, camera, WebRTC and Web Audio in the webview | present on both |
| `getDisplayMedia` (sharing the screen) | iOS has it; the Android WebView does not |
| `Notification`, `PushManager` | missing on both |
| Android, screen off for 2 min or in the background for 60 s, with or without a foreground service | timers, microphone packets and audio kept going (emulator; its microphone is silent) |
| iOS simulator, screen locked | WebKit ended the microphone track at once and suspended the page about 30 s later; after unlocking, the track stayed ended |
| iPhone, screen locked | a minute after locking, the page still ran and microphone packets still flowed; the test was stopped there |
| Android ring: self-managed ConnectionService with a full-screen call notification | rang with the app in the background; *Answer* brought the app forward |
| iOS ring through CallKit | the plugin's call was accepted; the simulator ends it at once because it has no call screen |
| iOS push registration | fails without the `aps-environment` entitlement, which needs the paid account. A push simulated with `simctl push` showed with the app closed |
| Android push registration (FCM) | not tried end to end: there was no Firebase project or phone. Without `google-services.json`, it fails cleanly |

**Keep Tauri.** Capacitor wraps the same two webviews, so every limit above would be the same there. Both reach Swift and Kotlin through plugins, and this plugin reached the call and push APIs that CS-144 and CS-145 need. Tauri also keeps one project for the desktop and the phones.

The simulator and the phone disagree about a locked screen. The question was handed to the app itself: CS-144 runs a real huddle with the screen locked for ten minutes on both phones. Whatever fails there is written down as a known limitation, not a reason to change frameworks. If iOS does cut the microphone, the way around it is the huddle's media in native code (LiveKit's Swift SDK) with the UI still in the webview.

What the phone apps must handle because of this:
- **Notifications go through the plugin, not the Web Notifications API**, as on desktop (CS-108).
- **No screen sharing from Android.** Watching someone else's screen works.
- **Android 16 draws under the status bar** (edge to edge). The test page pads with `env(safe-area-inset-top)` and was still covered, so the app has to pass the insets in.
- **A ring needs a push.** iOS freezes a locked app within about 30 s, so a timer in the app cannot ring later; on iOS a huddle rings from a VoIP push (PushKit). Tauri has no hook for the push callbacks on the app delegate, so the plugin adds them to Tauri's `AppDelegate` at run time.
- **A free Apple ID can sign the app**, for seven days at a time. `tauri ios build` builds for a generic device, so Xcode never registers the phone with the team. One `xcodebuild -destination id=<phone> -allowProvisioningUpdates` creates the profile; after that, the Tauri build signs.

## Mobile, later

Ticketed on 2026-11-04 as Wave 33 (then CS-122 – CS-126), for 5.0.0, and renumbered on
2026-11-13 as Wave 37 (CS-141 – CS-145), after the Slack gaps of 4.8.0. Decided on 2026-11-04 to build
the phone apps on Tauri 2 too, as iOS and Android targets of the desktop project, if the
spike (CS-142) finds that a huddle holds in the phone's webview and that a Tauri plugin can
receive a push; Capacitor around the same React client is the way around it if not. The
spike kept Tauri (above). What
mobile needs that desktop does not:

- **A bundled frontend.** A store app that only loads a remote page is likely to be rejected
  in review, so the app carries its own copy of the web client (React Native would mean
  writing the UI again). A bundled client talks to servers of other
  versions, so the API needs a compatibility policy first: this is when the deferred
  `/api/v1` decision in the [roadmap](./ROADMAP.md) comes back.
- **A push relay.** APNs and FCM accept notifications only from the app's publisher, so a
  self-hosted server cannot reach a phone directly. A relay run by the publisher forwards
  an ID and an encrypted payload; the server never hands it message text in the clear.
- **Tokens in the Keychain / Keystore** instead of local storage, CORS for the app's origin,
  and SSO that returns to the app through a deep link (shared with CS-109).
- **CallKit / ConnectionService** so a huddle can ring.

## Open decisions

- **Code signing.** Who holds the Apple Developer account (99 $ a year, also needed for
  iOS) and the Windows certificate (Azure Trusted Signing, around 10 $ a month), and when.
  Nothing in Wave 29 waits on it.
- **The push relay** (CS-145). Whether the project runs one. Without it, mobile stays the PWA on
  iOS and an Android build that receives push through the instance itself.
