# CS-133 — Clips: recorded audio, video and screen

**Wave:** 35 — huddles and clips (2026-11-13)
**Area:** frontend · backend · desktop
**Blocked by:** —
**Blocks:** —

## Problem

Showing a bug or explaining a design takes a huddle that somebody has to join right then, or
a screen recording made in another app and uploaded. Slack clips record audio, video or the
screen from the composer and send them as a message. ROADMAP had clips under *left for much
later*; they were chosen for this release on 2026-11-13.

## Approach

- **Recording:** a *Record a clip* button in the composer, next to attachments, offers:
  - audio only;
  - camera and microphone;
  - the screen with the microphone, and the camera in a bubble if wanted.
  - It uses `MediaRecorder` with the camera, microphone and screen code of the huddle
    (`features/huddle/lib/media.ts`).
  - A clip is at most five minutes, with a countdown in the last thirty seconds.
  - After stopping you preview, record again or send, with text if wanted.
- **The format:** record in a container and codec that plays everywhere the app runs:
  Chrome, Firefox, Safari, and the desktop app on macOS (WKWebView) and Windows (WebView2).
  - Check that first. If one recording format does not play in all of them, record as MP4
    where `MediaRecorder` offers it, and WebM elsewhere only if WebM plays in all of them.
  - Write down what was found.
- **Upload** goes through the streaming upload that exists, with the cap of CS-011, as a file
  marked `clip`. Its metadata holds:
  - the duration;
  - a poster frame the browser captures and uploads alongside;
  - for audio, a waveform of 100 peaks worked out in the browser.
- **Playback** in the message:
  - a player with the poster or the waveform, the duration, and 1×, 1.5× and 2× speeds;
  - range requests (`files/ranges.rs`), so seeking does not download the whole clip;
  - in the gallery, video clips sit with images (CS-055).
- **Retention, erase, export and file search** treat a clip as the file it is.
- **Not in this ticket:** transcripts and captions. That needs speech recognition beside
  the stack, the same question as live captions, which was not chosen.

## Acceptance

- [ ] A 20-second screen clip with the camera bubble, recorded in Chrome, plays in Safari, in
  Firefox and in both desktop apps, and seeks to the middle without loading it all.
- [ ] An audio clip shows its waveform and plays at 2×.
- [ ] A clip stops by itself at five minutes, and can still be previewed and sent.
- [ ] Denying the microphone or the screen says what was denied, and nothing is uploaded.
- [ ] Erasing the person removes their clips.

## Tests

`http_tests`: the `clip` metadata validated, and a poster that is not an image refused.
Frontend: the recorder's states, with `MediaRecorder` faked. E2E: record a clip from a fake
media stream, send it and play it in a second context.
