# CS-126 — Profile fields of the workspace's own

**Wave:** 33 — channels and the people in them (2026-11-13)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

A profile has a name, a title, pronouns, a time zone and a status. A company of forty asks
*who is in Finance*, *who does Sara report to* and *who sits in Belgrade*. The app has
nowhere to say it. Slack lets an admin add fields to every profile and filter the
directory by them.

## Approach

- **Fields an admin defines** in *Workspace settings → Profile fields*, ordered by dragging:
  - Each field has a label, an optional hint and a kind:
    - `text`;
    - `link`;
    - `date`;
    - `options`: a list the admin writes;
    - `person`: another member, such as a manager.
  - A field can be marked *admins only*, so members see it but cannot edit their own, for
    an employee number.
  - At most 50 fields.
- **Values** live in their own table, `(workspace_id, user_id, field_id) → jsonb`. A value
  is checked against its field's kind when it is written. Deleting a field deletes its
  values, after a confirmation that says how many.
- **Where they show:**
  - each person's profile panel, under the title;
  - *Edit profile*, for the fields they may edit;
  - the hover card, for the first two fields the admin picks to show there.
  - A `person` value links to that profile; a `date` uses the reader's locale.
- **The directory:**
  - *People* filters by an `options` field and by `person`, as in *reports to*.
  - Its search box matches `text` values.
  - Filters sit in the query string, so a filtered view can be shared.
- **Bots:** `users:read` returns the values, keyed by field id, with a route listing the
  fields.
- **Export and erase** include the values (CS-121j). A person who leaves keeps none.

## Acceptance

- [ ] An admin adds *Department* (options), *Manager* (person) and *Office* (text). A member
  fills them in, and they show on the profile and in the hover card.
- [ ] *People* filtered by *Department = Finance* lists exactly those members, and *Manager
  = Ana* lists her reports.
- [ ] A member cannot write an *admins only* field; the route answers `403`.
- [ ] A value of the wrong kind, an unknown option or a person outside the workspace is
  refused with the field's name.
- [ ] An export of a person includes their values, and erasing them removes them.

## Tests

`http_tests`:
- field CRUD and its authorization matrix;
- value validation per kind;
- the directory filters;
- `users:read` with values;
- export and erase.

Frontend: the field editor and the filter bar. E2E: define a field and filter by it.
