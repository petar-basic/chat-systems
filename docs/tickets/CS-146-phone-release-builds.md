# CS-146 — Phone release builds

**Wave:** 37 — mobile apps, released as 5.0.0
**Area:** mobile · ops
**Blocked by:** CS-144, and the decision on who holds the Apple Developer account and the Play upload key
**Blocks:** —

## Problem

The phone apps (CS-144) are built by hand from a checkout. TestFlight takes only a build
signed with a paid Apple Developer account, and Play's internal track only an `.aab`
signed with the project's upload key. Neither exists yet, so a release cannot produce
either build. Taken out of CS-144 on 2026-11-24 for that reason.

## Approach

- After each release, the release workflow builds an `.ipa` for TestFlight and an `.aab`
  for Play's internal track, signed with the project's accounts held as repository secrets.
- Store listing is a separate step the maintainer takes.
- The decision this waits on is in [native-apps-plan.md](../native-apps-plan.md) under
  *Open decisions*: who holds the Apple Developer account and the upload key.

## Acceptance

- [ ] A tagged release produces both builds and uploads them to TestFlight and Play's
  internal track.

## Tests

The release workflow on a tag, and the builds installed from TestFlight and Play.
