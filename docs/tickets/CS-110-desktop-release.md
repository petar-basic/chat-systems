# CS-110 — Desktop builds with every release

**Wave:** 29 — a desktop app (2026-11-02)
**Area:** desktop · ops · docs
**Blocked by:** —
**Blocks:** —

## Problem

An app nobody can download, or that never updates, is not shipped.

## Approach

- `release.yml` builds the app for macOS (universal) and Windows (NSIS installer) on a tag
  and attaches them to the GitHub release.
- Tauri's updater checks the latest release; updates are verified against the project's
  update-signing key, held as a repository secret.
- Code signing with an Apple Developer ID and a Windows certificate is a separate decision
  ([plan](../native-apps-plan.md#open-decisions)); until it is made the README says how to
  open an unsigned app on each system.
- README and CONTRIBUTING describe the app, where to get it, and how to build it from source
  with your own name and icon.

## Acceptance

- [ ] A tagged release has a macOS and a Windows download.
- [ ] An installed app updates itself to the next release.

## Tests

CI builds both targets on a pull request that touches `desktop/`.
