# CS-121p — Emails that look like they come from a company

**Wave:** 32b — email (2026-11-12)
**Area:** backend · frontend
**Blocked by:** CS-121o
**Blocks:** CS-121q

## Problem

Seen in MailHog on 4.7.0 and in `auth/service/email.rs` and `notifications/email.rs`:

- **Six of the seven emails are plain text.** Only the invite has an HTML part
  (`templates/invite_email.html`). The password reset, the sign-in code, the SSO reminder,
  the expired invite, the mention digest and the admin's test email are a few lines of
  `format!` text.
- **Subjects that do not say where they are from.** *Reset your password*, *You sign in with
  SSO* and *Your invitation has expired* name no instance and no workspace.
- **The reset email assumes you asked.** It opens with *You requested a password reset* even
  when somebody else typed the address, and does not say what to do then.
- **The digest has no way out.** Its link is three UUIDs long, nothing in it leads to the
  setting that sends it, and it has no `List-Unsubscribe`. Gmail and Yahoo ask for one-click
  unsubscribe (RFC 8058) on mail of this kind, and a mail without it is more likely to be
  marked as spam.
- **No `Message-ID`.** lettre does not add one, and MailHog's headers show it adding its own.
  Spam filters count a missing one against the mail, and a relay that adds one uses its own
  domain.
- **No `Auto-Submitted`.** Out-of-office replies answer the `noreply` address.

## Approach

- One askama layout every email extends (`templates/email/layout.html`):
  - the instance's icon, from `INSTANCE_ICON_URL`, else `{public_url}/icons/icon-192.png` when
    the public URL is HTTPS, and its name;
  - a heading and a few short paragraphs;
  - an optional button with the link written out under it, and an optional code in large
    spaced type;
  - a footer saying who sent it, to which address, and why.

  Tables and inline styles, at most 520 px wide, readable at 320 px and when a client darkens
  it.
- A text part written from the same values, with the link on a line of its own.
- Every email moves to it: reset, sign-in code, SSO reminder, expired invite, invite, digest
  and test email. Each one's strings live in `backend/api/locales/en.json` under `email.*` and
  are read through `notifications::text` in English, so CS-121q only has to pick the
  language.
- **Subjects** name the instance or the workspace: *Reset your {instance} password*,
  *{code} is your {instance} sign-in code* (unchanged: the E2E reads it), *Sign in to
  {instance} with SSO*, *Your invitation to {workspace} has expired*.
- **The reset text** says somebody asked to reset the password for the address. If it was not
  you, you can ignore it, and your password stays as it is.
- **The digest:**
  - a button to the message or the workspace;
  - a footer line naming the workspace whose setting sends it, linking to *Preferences →
    Notifications*;
  - `List-Unsubscribe` and `List-Unsubscribe-Post: List-Unsubscribe=One-Click`, pointing at
    `/api/email/unsubscribe/{token}`. The token is signed with the instance's secret and
    names the person and the workspace.

  A `POST` there turns off that person's mention emails in that workspace without signing
  in. A `GET` shows a page with a button that does it, so a link scanner opening the URL turns
  nothing off. What the digest says stays as it is: who, where and a link, never what was
  said.
- **Headers:** every email gets `Auto-Submitted: auto-generated`, and a `Message-ID` built
  from the outbox row's id and the domain of `SMTP_FROM_ADDRESS`, so a retry sends the same
  one. The outbox stores the unsubscribe URL with the email.

## Acceptance

- [ ] Every email has a text and an HTML part in the same layout, checked in MailHog at 320 px
  and at desktop width.
- [ ] Every subject names the instance or the workspace.
- [ ] Every email has a `Message-ID` on the sender's domain, the same across retries, and
  `Auto-Submitted`.
- [ ] A one-click `POST` to a digest's unsubscribe URL turns its mention emails off, and
  *Preferences → Notifications* shows them off. A `GET` changes nothing until the button is
  pressed, and a forged or altered token is refused.
- [ ] Everything interpolated into the HTML is escaped.

## Tests

Template tests: escaping, the icon rule, the text part carrying the link. `http_tests`: each
email's subject and parts in the outbox, the headers, unsubscribe by `POST` and not by
`GET`, a forged token. E2E: the sign-in code and invite specs still read their emails.
