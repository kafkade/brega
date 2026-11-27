# Handoff: capacity planning and weekly review

Status: **proposal, not started**. Origin: a planning session (2026-11-26) that
folded a separate "almanac" dashboard idea into tock. Delete this file once the
work is tracked in issues/ADRs.

## Problem

One person, many roles (day job, household, kids, side businesses, personal),
24 hours. Tasks need a place on the calendar or they don't get done
(time-blocking, à la Cal Newport). What's missing is a view of **demand vs.
capacity** across roles, so nothing falls through the cracks and over-commitment
is visible on Monday, not Friday.

## Decisions

1. **No separate dashboard app.** Capacity planning and the weekly review live in tock.
2. **Division of responsibility:** the calendar owns *when*, tock owns *what* and
   *how it fits*. tock does not become a calendar app.
3. **Reading calendars is allowed, but only free/busy.** This reverses an earlier
   "tock never pulls calendars" stance. tock reads busy intervals (CalDAV or ICS
   subscription URLs) to compute capacity. It does not store or display other
   people's event details. Work calendars are often policy-restricted and
   subscribed read-only; busy-only keeps that safe.
4. **Areas are roles.** One area per role. No new "role" concept.
5. **Product-level only.** Build only what is generally useful to tock users;
   one user's personal quirks stay in local config or scripts.

## Already in tock (reuse)

- Recurring tasks (`--recur`), projects and areas (`domain/area.rs`), urgency
  scoring, time tracking, the TUI.
- `tock-caldav`: pure iCal parser, Task↔VTODO and TimeBlock↔VEVENT mapping, and a
  sync engine behind a `CalDavTransport` trait (architecture §9.5).

## Gaps (proposed work, in order)

| # | Gap | Notes |
|---|-----|-------|
| 1 | **Task effort estimate** | No estimate/duration field on Task today. Needed for demand. Event-sourced change, so check the vault format versioning policy (ADR-013). |
| 2 | **Area weekly budget** | e.g. `tock area mod rental --budget 3h/w`. Optional; unset means "no budget." |
| 3 | **Busy-time source** | Config for ICS/CalDAV URLs and read-only FREEBUSY extraction. Needs an HTTP transport (none exists yet; tock-caldav is I/O-free). Decide which crate owns I/O (likely tock-cli), and how credentials and URLs are stored encrypted. |
| 4 | **`tock review`** | Weekly view per area: budget vs. tracked vs. planned, overdue, slipped (rescheduled ≥2×), areas with zero planned time, big rocks without a block. |
| 5 | **`tock plan`** | Free hours (from #3) vs. estimated demand (#1), ranked by urgency + area budget deficit. Proposes blocks, then on confirm pushes VEVENTs to a chosen calendar via tock-caldav. Never writes to read-only/work calendars. |
| 6 | **Urgency input** | Optional urgency coefficient for an area's budget deficit; must stay explainable in `tock urgency`. |

Suggested MVP: #1 + #2 + #4 (no network). Then #3, then #5.

## Open questions

- An ADR is needed for decision 3 (external calendar reads under the E2EE / zero-I/O-core model).
- Estimates: a single number, or min/likely? How should missing estimates be treated (default per area?)
- Should "slipped" be derived from the event log (reschedule events) or tracked explicitly?
- `tock plan` UX: CLI table plus confirm, a TUI pane, or both?
