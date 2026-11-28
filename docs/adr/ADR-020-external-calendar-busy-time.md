# ADR-020: Read-only busy/free time from external calendars

**Status:** Accepted
**Date:** 2026-11-26

> Records decision 3 of [the capacity-planning handoff](https://github.com/kafkade/brega/blob/ed10ad4/docs/handoff-capacity-planning.md)
> (removed once its work was tracked in #228–#231, #240, and #246).
> Narrows the role of external calendars described in
> [architecture §9.5](../architecture.md#95-caldav-integration): §9.5 treats
> CalDAV as an optional *outbound/two-way projection* of tasks and time blocks;
> this ADR adds a separate, strictly **inbound, read-only, busy/free-only**
> input used to compute capacity. It does not change §9.5's write path, the
> vault format ([ADR-013](ADR-013-vault-format-versioning-policy.md)), or the
> sync protocol ([ADR-003](ADR-003-event-sourced-sync.md)).

## Context

Capacity planning (handoff gaps #3–#5) compares **demand** (task effort
estimates, area weekly budgets) with **capacity** (hours actually free this
week). Capacity cannot be computed honestly without knowing when the user is
already committed — meetings, school runs, appointments — and that information
lives in the user's calendars, not in the vault.

The working rule had been that the app never pulls calendars. The handoff
reverses that, with two limits: the calendar keeps owning *when*, the app owns
*what* and *how it fits*; and only free/busy information is read.

Forces at play:

1. **Third-party data.** Calendar events include other people's names, meeting
   titles, locations, and notes. Work calendars are frequently subscribed
   read-only and governed by employer policy. Persisting or syncing those
   details — even end-to-end encrypted — would move third-party data into the
   user's vault and onto every synced device and backup.
2. **Zero-I/O core** ([ADR-001](ADR-001-rust-zero-io-core.md)). Calendar access
   is I/O (OS frameworks, HTTP) and must stay at the platform edge.
3. **Encryption boundary** ([ADR-002](ADR-002-end-to-end-encryption.md)).
   Anything that does enter the vault is encrypted and synced; anything that
   should not be synced must stay out of the synced domain tables.
4. **Platform direction.** The primary clients are now the iPhone and Mac apps.
   On Apple platforms the OS already aggregates the user's calendar accounts
   (iCloud, Exchange/Microsoft 365, Google, CalDAV, subscribed ICS) behind
   **EventKit**, with the user's existing credentials and an OS permission
   prompt. A direct CalDAV/ICS client would duplicate that and require the app
   to store calendar credentials.
5. **Existing code.** `brega-caldav` is I/O-free and models VTODO/VEVENT
   mapping and two-way sync; it has no free/busy extraction and no network
   transport. The CLI exposes a CalDAV entry point whose network sync was never
   wired.

## Decision

### 1. Busy/free only, read-only, never written

The app reads calendars solely to obtain **busy intervals**. For each event it
uses only:

| Used | Not used (never read into app state, logged, stored, or synced) |
| --- | --- |
| start, end, all-day flag | title, notes, location, URL |
| availability (busy / tentative / free / unavailable) | attendees, organizer, alarms |
| an opaque calendar identifier (for the user's include/exclude choice) | attachments, recurrence details beyond expanded occurrences |

Reading never creates, modifies, or deletes calendar data. Writing proposed
blocks to a calendar (handoff gap #5, `plan`) is **out of scope** for this ADR
and requires its own decision; when it comes, it must never target a calendar
the user marked read-only or "work".

Interpretation rules:

- `busy` and `unavailable` count as busy. `free` is ignored.
- `tentative` counts as busy by default (conservative capacity); a per-device
  setting may change it.
- All-day events are ignored by default (they usually mark days, not
  occupancy); a per-calendar setting may count them as busy for the whole
  availability window.
- Overlapping intervals are merged before use.

### 2. Sources and where the I/O lives

| Source | Client | Status |
| --- | --- | --- |
| **EventKit** (all calendars configured on the device) | iPhone/Mac app, Swift edge | **First source** |
| ICS subscription URL (HTTP GET, parse VEVENT/VFREEBUSY) | Any client edge (Swift `URLSession`; dormant CLI via `reqwest`) | Later |
| CalDAV `free-busy-query` / time-range `REPORT` | Client edge | Later |

The **core stays I/O-free.** The platform edge converts events to a minimal
value type and passes it into Rust:

```rust
/// A merged, opaque busy interval. Carries no event content.
pub struct BusyInterval {
    pub start: OffsetDateTime, // UTC
    pub end: OffsetDateTime,   // UTC, end > start
}
```

Capacity is computed by a **pure function** in `brega-core` (e.g.
`capacity::free_time(window, availability, &[BusyInterval]) -> FreeTime`), unit
tested without any calendar. Parsing of ICS/VFREEBUSY for the later sources
lives in `brega-caldav` (pure); fetching lives at the edge.

### 3. Nothing calendar-derived enters the synced vault

- Busy intervals are **computed on demand and held in memory only**. They are
  not written to domain tables, the event log, undo log, backups, or sync
  payloads. Each device reads its own calendars.
- Calendar selection (which calendars count, tentative/all-day handling) is
  **device-local state**: stored outside the synced tables (today's
  `app_state` table is excluded from sync), because calendar identifiers differ
  per device and the selection may legitimately differ (e.g. the work calendar
  exists only on the Mac).
- Results that the user explicitly saves (e.g. a weekly-review summary they
  choose to keep) may store **aggregate minutes only** — never intervals or
  event fields.

### 4. Credentials (later sources)

ICS URLs routinely embed secret tokens and CalDAV needs passwords. When those
sources are added, the URL/credential is stored in the **platform keystore**
(Keychain on Apple), never in the vault or in sync, and is never logged. With
EventKit the app stores no calendar credentials at all.

### 5. Permission and failure behavior

- Calendar access is **opt-in**: requested only when the user enables
  "Use my calendars for capacity", with a usage description stating that only
  busy times are read. On Apple, reading requires full calendar access; the
  app requests it only at that moment.
- Denied, revoked, or unavailable access degrades to "capacity unknown": the
  review still shows budgets, tracked, and planned time; nothing fails.
- Capacity views show which calendars were included and when they were read,
  so a missing calendar is visible rather than silently over-reporting free time.

## Consequences

**Positive:**

- Capacity becomes computable from real commitments without the app becoming a
  calendar or storing third-party event content.
- EventKit reuses accounts the user already configured, including policy-managed
  work accounts, with no credentials stored by the app.
- No vault-format, schema, or sync change is required; the core remains pure and
  testable with synthetic intervals.
- Safe default for work calendars: read-only, busy-only, never synced.

**Negative:**

- Full calendar access on Apple technically grants the app read access to event
  details; the busy-only guarantee is enforced by app code and tests, not by the
  OS. A review/test must assert no event fields are retained.
- Capacity can differ between devices (e.g. a calendar present only on the Mac).
  This is intentional but must be shown in the UI.
- Busy intervals are recomputed each time; acceptable at personal scale.

**Neutral:**

- ICS/CalDAV sources and calendar writes remain future, separately decided work.
- The existing `brega-caldav` two-way design (§9.5) is unaffected and stays
  dormant with the CLI.
