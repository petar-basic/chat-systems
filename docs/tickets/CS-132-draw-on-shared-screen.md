# CS-132 — Drawing on a shared screen

**Wave:** 35 — huddles and clips (2026-11-13)
**Area:** frontend · realtime · desktop
**Blocked by:** —
**Blocks:** —

## Problem

When somebody shares their screen in a huddle, the others can only point with words: *the
button left of that, no, below it*. In Slack anyone in the huddle can draw on the shared
screen, and the strokes fade after a few seconds.

## Approach

- **Drawing is on by default** while a screen is shared. The person sharing can turn it off
  from the control bar.
- **A viewer** picks the pen on a shared screen's tile and draws with the mouse or a finger:
  - Strokes are in the viewer's colour, with their name at the start.
  - Each stroke fades three seconds after it ends.
  - Esc and the pen button both stop drawing.
- **The strokes travel as a realtime frame,** `huddle.draw`, like `huddle.reaction`, so the
  same code serves a mesh huddle and one on the SFU:
  - Points are in coordinates relative to the shared video (0 to 1 on each axis), so they
    land in the same place at every tile size.
  - Points are batched every 50 ms. A frame carries at most 64 points, well under the
    gateway's 64 KiB limit.
  - The gateway accepts them only from someone in that huddle, and only while drawing is on.
  - The rate is bounded per person like other huddle frames.
- **Where they show:**
  - every viewer's tile of the shared screen;
  - the sharer's own preview tile;
  - in the desktop app, a transparent, click-through window over the shared display, so
    the sharer sees them on the real screen.
  - Before building the desktop window, check that Tauri can make it click-through on macOS
    and Windows. Where it cannot, the sharer sees strokes only in the huddle window, and the
    ticket says which OS that is.
- **A huddle of one viewer and one sharer** draws the same way. Nothing is recorded or kept.

## Acceptance

- [ ] Ana shares her screen and Bob draws a circle. Ana's preview, Ana's real screen in the
  desktop app, and Cleo's tile all show it in Bob's colour, and it fades after three seconds.
- [ ] Ana turns drawing off. Bob's pen disappears and the gateway drops his frames.
- [ ] Drawing works the same in a mesh huddle and on the SFU.
- [ ] Somebody not in the huddle cannot send `huddle.draw`; the gateway drops it.

## Tests

Realtime: who may send `huddle.draw`, the switch and the rate bound. Frontend: coordinate
mapping at two tile sizes, batching and fading. E2E: two contexts with a fake screen share,
one draws and the other sees the overlay.
