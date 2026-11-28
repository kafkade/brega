# Roadmap after v1

Scope for the first release lives in [release-v1.md](release-v1.md), which wins
over this file on anything in v1. Items here are either **agreed** (decided, order
may still change) or **possible** (not committed). No dates are implied.

## Agreed

| Item | Notes |
| --- | --- |
| v1.1 — Calendar busy time | EventKit, busy/free only, per [ADR-020](adr/ADR-020-external-calendar-busy-time.md). Shows free hours vs estimated demand in the Weekly Review. |
| Capacity `plan` | Propose blocks from free time and demand; writing to calendars needs its own ADR and must never target read-only/work calendars. |
| Area-deficit urgency input | Optional and explainable in the urgency breakdown. |
| Projects and tags | Schema already exists; additive. |
| Recurrence | Schema already exists; additive. |
| Widgets and App Intents | Existing Swift source can be reused. |
| Public App Store release | Requires an external security review first. |

## Possible (uncommitted)

- CLI revival on the shared core (dormant code retained).
- Self-hosted server sync (dormant `brega-server`, SRP accounts).
- Password + Secret Key wrap of the vault key, for non-Apple clients (additive to
  the format).
- ICS/CalDAV busy-time sources.
- watchOS app, iPad-specific layouts, web client, paid hosted sync.

## Delivery order for v1 (current plan)

1. Rename tock → brega across code, identifiers, and GitHub references (#226; done).
2. Sync forward-compat fix (R-13).
3. Estimate, area budget, reschedule log, Weekly Review core (R-10…R-12).
4. Vault format v3 with SQLCipher and the iCloud Keychain key model (N-02, N-03).
5. Containment of dormant code and CI gates (N-06).
6. Single multiplatform app shell, onboarding, and unlock (R-01, N-05).
7. Tasks, areas, focus, timers, habits screens (R-02…R-06), Weekly Review screen.
8. Automatic conflict merge, CloudKit transport, recovery, and backup (R-07…R-09).
9. Performance, accessibility, and the release gate (N-07, N-08).
