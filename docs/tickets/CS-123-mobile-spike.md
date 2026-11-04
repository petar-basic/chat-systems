# CS-123 — Does a Tauri phone app hold a huddle and receive a push?

**Wave:** 33 — mobile apps (2026-11-04), released as 5.0.0
**Area:** mobile
**Blocked by:** —
**Blocks:** CS-125 · CS-126

## Problem

The desktop app is Tauri 2 (CS-106, CS-107), and Tauri 2 builds for iOS and Android, so the
phone apps can be the same project. Two things could make that the wrong choice, and they
are cheaper to find out before the app is built than after: whether a huddle works inside
the phone's webview (WKWebView on iOS, the system WebView on Android), and whether a Tauri
plugin can receive a push from APNs and FCM.

## Approach

- A throwaway Tauri 2 mobile app that loads the web client and joins a huddle on staging:
  microphone and camera permissions, audio with the screen locked and with the app in the
  background, LiveKit for a large call, Bluetooth headphones.
- A small Tauri plugin, in Swift and in Kotlin, that registers for push and shows the token
  in the page; a push sent by hand to APNs and FCM arrives with the app closed.
- Whether CallKit (iOS) and ConnectionService (Android) can ring a huddle from that plugin.
- The findings go into [native-apps-plan.md](../native-apps-plan.md), as CS-106's did. If
  either fails, the plan names the way around it, Capacitor around the same React client
  being the one already written down.

## Acceptance

- [ ] A written answer, on a real iPhone and a real Android phone, to: does a huddle hold
  for ten minutes with the screen locked, and does a push arrive with the app closed.
- [ ] A recommendation to keep Tauri for mobile or not, with the reason.

## Tests

None; this is a spike. What it leaves is the plan's new section and the decision.
