# CS-106 — Does a Tauri shell hold a huddle?

**Wave:** 29 — a desktop app (2026-11-02)
**Area:** desktop
**Blocked by:** —
**Blocks:** CS-107

## Problem

The [plan](../native-apps-plan.md) puts each server's web app inside a Tauri 2 window. On
Windows that window is WebView2, which is Chromium and does what Chrome does. On macOS it is
WKWebView, whose support for `getUserMedia` and `getDisplayMedia` inside an app is the one
thing the plan cannot assume. A desktop app that cannot hold a huddle or share a screen is
not worth shipping, so this is checked before anything else is built.

## Approach

A throwaway Tauri 2 app on macOS with one window pointed at a server URL, the camera and
microphone usage descriptions in `Info.plist`, and the web view's media permission prompts
answered. Against the staging server:

- Sign in with a password, reload, and stay signed in.
- Join a huddle with the microphone and the camera, with someone in a browser on the other
  side, in the mesh and after promotion to LiveKit.
- Share the whole screen and one window.
- Receive a message while the window is in the background.

## Acceptance

- [ ] Each check above is recorded as works / fails, with the macOS and Tauri versions.
- [ ] A decision: Tauri, or Electron with the reason.

## Tests

None; this is a spike. Its findings go into this ticket's PR and into the plan.
