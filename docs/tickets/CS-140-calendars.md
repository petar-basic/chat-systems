# CS-140 — Google and Outlook calendars

**Wave:** 36 — apps that do more (2026-11-13)
**Area:** backend · frontend · ops · docs
**Blocked by:** CS-130 · CS-134
**Blocks:** —

## Problem

Nothing knows when somebody is in a meeting. People get pinged mid-call, set *In a meeting*
by hand and forget to clear it, and a meeting starts without a nudge. Slack's Google
Calendar and Outlook Calendar apps do four things:
- set the status during meetings;
- remind a minute before;
- send a morning agenda;
- show who is out of office.

## Approach

- **Two providers** in the connected-apps framework of CS-053, `google_calendar` and
  `outlook_calendar`:
  - A workspace admin turns each on with an OAuth client: a Google one, which may be the
    Drive client with one more scope, and a Microsoft Entra one.
  - Each person connects their own calendar under *Preferences → Calendar*.
  - The scopes are read-only: Google `calendar.events.readonly`, and Microsoft Graph
    `Calendars.Read` and `offline_access`.
- **Sync in `chat-worker`:**
  - Each connection is polled every five minutes for the next 24 hours of events,
    incrementally: Google's sync token, Graph's delta query.
  - Only what is needed is stored: start, end, title, the join link and whether the event
    is private.
  - Events older than a day are deleted. Disconnecting deletes everything stored for that
    person.
- **What it does,** each part a switch in *Preferences → Calendar*:
  - **Status:** while an accepted meeting runs, the status is *In a meeting* with 🗓️, and
    clears afterwards. It is never set over a status the person wrote. An out-of-office
    event sets *Out of office* and away until it ends.
  - **Reminder:** a DM from the calendar's bot one minute before an accepted event. It
    carries the title, the time and a *Join* button for the Meet, Teams or Zoom link.
  - **Agenda:** a DM at a time the person picks, in their time zone, listing the day's
    events.
  - **Private events** show as *Busy* in all three.
- **The bots** are built-in bots of CS-130, *Google Calendar* and *Outlook Calendar*. Their
  messages use the blocks of CS-134, with link buttons only.
- **Erase and export** cover connections and stored events.
- **`docs/RUNBOOK.md`** says how to make both OAuth clients.

## Acceptance

- [ ] Ana connects Google Calendar. During her 10:00 meeting her status reads *In a meeting*,
  and at 10:30 it clears.
- [ ] At 9:59 she gets a DM with *Join* opening the Meet link.
- [ ] Bob with Outlook gets the same, and his out-of-office day shows him away with *Out of
  office*.
- [ ] A status Ana wrote herself is never replaced.
- [ ] A private event shows as *Busy* with no title anywhere.
- [ ] Disconnecting removes her stored events, and the status stops changing.

## Tests

`http_tests` against fake Google and Graph servers:
- connect and disconnect;
- incremental sync with a changed and a deleted event;
- the status rules, including never over the person's own status;
- the reminder and the agenda through the loop with the clock moved;
- private events;
- erase.
