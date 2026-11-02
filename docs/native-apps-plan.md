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

- **The app does not bundle the frontend.** Each server's window loads that server's web
  app from its URL. The app is therefore always the same version as the server it talks
  to, whether that server runs 3.7 or 5.0, and no API compatibility promise is needed for
  desktop. It also means sign-in, cookies and the realtime socket are exactly what the
  browser does today: same origin, nothing new on the server.
- **Tauri 2, not Electron.** The project is Rust already, the installer is around 10 MB
  instead of around 150 MB, and the web view is the system's (WebView2 on Windows,
  WKWebView on macOS). The risk is WKWebView's support for camera, microphone and screen
  sharing in huddles; CS-106 checks it before anything else is built, and Electron is the
  fallback if it fails.
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

## Mobile, later

Not ticketed yet. What it needs that desktop does not:

- **A bundled frontend.** A store app that only loads a remote page is likely to be rejected
  in review, so the app carries its own copy of the web client (Capacitor around the existing React app; React
  Native would mean writing the UI again). A bundled client talks to servers of other
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
- **The push relay.** Whether the project runs one. Without it, mobile stays the PWA on
  iOS and an Android build that receives push through the instance itself.
