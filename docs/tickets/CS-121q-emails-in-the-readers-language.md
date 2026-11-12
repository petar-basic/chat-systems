# CS-121q — Emails in the reader's language

**Wave:** 32b — email (2026-11-12)
**Area:** backend
**Blocked by:** CS-121p
**Blocks:** —

## Problem

Since CS-121m the mention digest and push notifications follow the language set in
*Preferences → Language & region*. Every other email is English, whatever the reader
chose. A team that uses the app in Japanese signs in with an English code email and resets
its password in English.

## Approach

- **Choosing the language:**
  - The person's language from `users.preferences` comes first.
  - While it is *Same as my browser*, the `Accept-Language` of the request that caused the
    email counts, when it names a language the app has. That request comes from the person's
    own browser: *Forgot password*, *Email me a code*.
  - Otherwise the email is in English.
  - The digest has no request behind it and keeps CS-121m's rule.
- **An invite** is in the inviting admin's language, by the same rule, since the person
  invited has no settings yet. An invite sent again from *Email me a code* or *Forgot
  password* follows that request.
- The layout from CS-121p takes the language and sets `lang` on the HTML. A string the
  language lacks falls back to English, one string at a time, as `notifications::text` does
  already.
- New keys go into `en.json` only; the Japanese is the translator's.

## Acceptance

- [ ] A person whose language is Japanese gets every email with the Japanese strings that
  exist, and English for the rest.
- [ ] With *Same as my browser* and a Japanese browser, *Forgot password* and *Email me a code*
  send the email in Japanese.
- [ ] An invite is in the language of the admin who sent it.
- [ ] Without a language anywhere the email is English, and the sign-in code subject still
  matches what the E2E reads.

## Tests

Unit: the language chosen from a preference, an `Accept-Language` header and neither.
`http_tests`: a reset and a code for a Japanese preference and for a Japanese browser; an
invite from a Japanese admin.
