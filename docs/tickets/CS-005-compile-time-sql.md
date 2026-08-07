# CS-005 — Decide on compile-time-checked sqlx queries

**Wave:** 0 — Safety net and structural groundwork
**Area:** backend
**Blocked by:** —
**Blocks:** nothing hard, but it touches every `repo.rs` — so it is either done here or
consciously never

## Problem

The backend uses `sqlx::query()` / `query_as::<_, T>()` in 139 places and
`sqlx::query!` / `query_as!` in **zero**. Every query is verified at runtime.

The correctness risk is real but bounded: 233 integration tests run against a real
Postgres with migrations applied, so a column rename does get caught — in CI, not at
`cargo build`. The cost is the strongest guarantee sqlx offers, given up by default
rather than by decision.

`SELECT *` compounds it. Roughly half the repo methods use `query_as::<_, T>("SELECT *")`
([example](../../backend/api/src/files/repo.rs#L44)), which binds the struct to whatever
column order the table happens to have and silently breaks when a migration adds a column
the struct lacks.

## Why here

This is a decision ticket, not a feature. If the answer is yes, it must happen before the
waves below, because every ticket from CS-006 onward writes new repo methods and they
should be written in the target style. If the answer is no, close it and stop paying the
review cost of re-litigating it in each PR.

## The decision

**Recommendation: adopt `query_as!` for new code, migrate opportunistically, do not do a
big-bang rewrite.**

Rationale: a full conversion of 139 call sites is a week of mechanical work with no
user-visible change and a real chance of introducing bugs in code that currently has
none. But leaving the codebase with two styles indefinitely is worse than either
extreme. A stated rule with a deadline is the middle path that actually converges.

The cost of adopting: the macros need `DATABASE_URL` at compile time or a checked-in
`.sqlx/` offline cache. That means one more CI step (`cargo sqlx prepare --check`) and
one more thing contributors can forget. That is the honest price.

## Approach

If adopted:

1. **Offline mode, checked in.** Run `cargo sqlx prepare --workspace` and commit
   `.sqlx/`. Add `cargo sqlx prepare --workspace --check` to the backend CI job so a
   query change without a regenerated cache fails the build. Document the regeneration
   command in `CONTRIBUTING.md` next to the existing test commands.
2. **New code only, from this ticket forward.** Every repo method added by CS-006 and
   later uses `query_as!`. Add the rule to `CONTRIBUTING.md` under Backend so it is a
   review checklist item, not folklore.
3. **Kill `SELECT *` as you go.** The macros force an explicit column list anyway;
   converting a method means naming its columns, which is the part that carries the real
   value.
4. **Opportunistic migration.** Any repo method touched for another reason gets converted
   in the same PR. Track the remaining count in this ticket; close it when it hits zero.

If rejected: record the reason here, and instead add a narrower guard — replace every
`SELECT *` with an explicit column list, which removes the largest share of the risk for
a fraction of the effort.

## Acceptance

- [ ] Decision recorded in this file with a date and a rationale.
- [ ] If adopted: `.sqlx/` committed, `cargo sqlx prepare --check` in CI, rule in
      `CONTRIBUTING.md`, remaining-conversions count tracked here.
- [ ] If rejected: `SELECT *` eliminated from all repos.

## Tests

No new behaviour. The existing suite must pass unchanged — that is the point.
