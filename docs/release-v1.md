# v1 release scope — source of truth

**Status:** Approved 2026-11-26 (owner). Product name **brega** (formerly tock; code and GitHub
repository renamed in #226).

## Which document governs

| Question | Governing document |
| --- | --- |
| What is in or out of the first release, and how readiness is judged | **This file** |
| What comes after v1, and whether it is agreed or only possible | [roadmap.md](roadmap.md) |
| Why consequential decisions were made | [ADRs](adr/README.md); the v1 product direction is [ADR-021](adr/ADR-021-iphone-mac-first-product-direction.md) |
| How the system is designed | [architecture.md](architecture.md) — design reference; where it disagrees with this file on scope, **this file wins** |

This file supersedes the earlier "1.0 = CLI + self-hostable server + web console"
scope (README, GitHub milestone *v1.0 — CLI + Self-Host GA*) and the release
roadmap in architecture §10. Requirement IDs below are stable; issues and tests
reference them.

## Target user and problem

- **User:** the owner first — a power productivity practitioner with an iPhone and
  a Mac, juggling several roles (work, household, family, side projects).
- **Problem:** tasks, focus sessions, time tracking, and habits live in separate
  tools with manual bridges, and over-commitment across roles shows up on Friday
  instead of Monday.
- **Core value:** one private, local-first app where focused work on a task
  records time and can count toward a habit, and a weekly review shows demand
  versus budget per area — on both devices.

## Essential journeys

| ID | Journey | Boundary |
| --- | --- | --- |
| J1 | First launch creates a vault, stores its key, shows the recovery kit once, then unlocks with Face ID / Touch ID. | No account, email, or server. |
| J2 | Capture a task, see Inbox/Today, complete it. | Title, notes, due date, planned day, estimate, area. No projects or tags. |
| J3 | Focus on a task with a countdown and end notification; focused time is logged. | Credited time = actual elapsed focus time. |
| J4 | Start/stop a timer on a task; see today's and this week's time. | Per-task and per-area totals. |
| J5 | Create a habit, log it, see the streak. | Daily cadence only. |
| J6 | Edits on the iPhone appear on the Mac and back, including offline edits on both. | Same Apple ID, CloudKit private database, ciphertext only. |
| J7 | Recover the vault on a fresh device from the recovery kit. | No server-side recovery. |
| J8 | Estimate tasks, give areas weekly budgets, open the Weekly Review. | Single-number estimates; unestimated tasks are counted, not guessed. |

## Requirements

### Functional

| ID | Requirement | Acceptance criteria |
| --- | --- | --- |
| R-01 | Vault creation and unlock (J1) | Fresh install creates a vault and shows the recovery kit once with an explicit "saved" confirmation; relaunch unlocks with biometrics; cancel or failure leaves data inaccessible. |
| R-02 | Tasks (J2) | Create, edit, complete, delete; set or clear a planned day (existing `scheduled_for`); Inbox and Today lists; data survives relaunch. |
| R-03 | Focus (J3) | Countdown continues while backgrounded; local notification at the end; pause and stop; the logged time block equals actual elapsed focus time (±1 s). |
| R-04 | Time tracking (J4) | At most one running timer; starting another stops the first; per-task and daily/weekly totals. |
| R-05 | Cross-domain links | Completing a task stops its running timer/focus; a completed focus session creates a time block linked to the task; a habit can opt in to "focus sessions count toward this habit". |
| R-06 | Habits (J5) | Create/archive, log once per day, current and best streak correct across day boundaries and time zones. |
| R-07 | iCloud sync (J6) | Changes appear on the other device once both are online; offline edits on both converge; no data lost; concurrent same-field edits merge automatically (latest edit wins, the other value kept in history). |
| R-08 | Recovery (J7) | Recovery kit plus iCloud data, or an encrypted backup, restores a working vault on a new device. |
| R-09 | Encrypted backup | Manual encrypted backup to Files; restore verified. |
| R-10 | Task effort estimate | Optional minutes, entered as `90m`, `1h30m`, `2h`; zero/negative rejected; syncs. |
| R-11 | Areas with weekly budget | Create/archive an area, optional weekly budget, assign a task to an area; unset budget means no budget. |
| R-12 | Weekly Review (J8) | Per area, for the current ISO week in the local time zone: budget, tracked, planned, overdue, slipped (moved later ≥2 times), zero-planned flag, unestimated count, high-priority tasks without a planned day. "Planned" = estimates of open tasks whose planned day falls in the week. Correct across DST and week boundaries. |
| R-13 | Mixed-version sync safety | A device on an older schema applies the known columns of a newer device's events and preserves unknown ones; upgrading emits no spurious updates. |

### Non-functional

| ID | Constraint | Acceptance criteria |
| --- | --- | --- |
| N-01 | Zero-knowledge transport | CloudKit records hold only ciphertext; an automated test asserts no plaintext marker in uploaded payloads. |
| N-02 | Encryption at rest | No task, habit, focus, or time plaintext readable from the vault file without the key (SQLCipher keyed by the vault key) plus OS file protection. |
| N-03 | Durable, extensible format | Vault format v3 (brega labels, SQLCipher layout) frozen before first real use; later features add fields/entity kinds through additive schema migrations, with no user action. |
| N-04 | Shared core | Domain logic, crypto, storage, and sync reconciliation in Rust; Swift holds UI and platform glue (Keychain, CloudKit transport, notifications, EventKit later). |
| N-05 | One app codebase | One multiplatform SwiftUI target for iPhone and Mac; Xcode project generated from committed config. |
| N-06 | CI | Rust tests, app builds for iOS Simulator and macOS, and app-level tests on every PR. |
| N-07 | Responsiveness | Lists render and actions complete in < 100 ms on a vault with 5,000 tasks and 50,000 events on the owner's devices. |
| N-08 | Accessibility baseline | VoiceOver labels on all controls, Dynamic Type, no meaning by color alone. |

Platform floor: iOS 26 / macOS 26. Distribution: development/TestFlight builds for
the owner.

## Explicitly not in v1

- CLI/TUI, self-hosted server, SRP accounts, web console, hosted or paid service.
- Projects, tags, recurrence, dependencies, user-defined attributes, contexts,
  saved reports, checklists, annotations, undo/redo.
- Calendar busy time (v1.1, [ADR-020](adr/ADR-020-external-calendar-busy-time.md)),
  capacity `plan`, area-deficit urgency.
- Widgets, Siri/App Intents, share extension, watchOS, iPad-specific layouts.
- Importers, Markdown export, CalDAV, languages other than English.
- Public App Store release and external security review (both required before a
  public release, not for this personal release).

## Release is ready when

1. R-01…R-13 and N-01…N-08 pass their acceptance criteria.
2. The owner has used the app daily on both devices for 14 consecutive days with
   no data loss.
3. A clean-device recovery (J7) has succeeded at least once.
4. No open issue labeled data-loss or security.

## Existing out-of-scope code (containment)

Out-of-scope code stays in the repository **dormant**: excluded from default
builds, release artifacts, and gating CI; labeled; removed later if unused.

| Area | Containment |
| --- | --- |
| `brega-cli` (incl. TUI), `brega-server`, `brega-wasm` + `apps/web`, `brega-account` (SRP), `brega-caldav`, `brega-import`, `brega-export` | Remove from `default-members`; `publish = false`; a `DORMANT.md` note per crate/app; no release artifacts; may build in a non-gating CI job. |
| `apps/ios`, `apps/macos`, `apps/watchos` | Reused as source for the single `apps/brega` target; marked dormant, then deleted once the new app covers them. |
| `release.yml` (CLI binaries, Homebrew), Docker/Helm/systemd deploy | Triggers disabled; documented as dormant. |
| Core crates the app uses (`brega-core`, `brega-crypto`, `brega-storage`, `brega-sync`, `brega-uniffi`) | Stay active. Capabilities used only by dormant entry points (e.g. SRP adoption paths) stay compiled but are not exposed through the app's UniFFI surface. |
| Gating CI job changes | Mirror in `kafkade/github-infra` (`repos/brega/main.tf`) or PRs block. |

## Dependencies, assumptions, risks

- **Dependencies:** Apple Developer Program membership (CloudKit, iCloud Keychain
  sync, signing); App Store Connect name and bundle ID reservation for "brega".
- **Assumptions:** one user, one Apple ID, two devices.
- **Risks:** "brega" means tacky/cheesy in Brazilian Portuguese; App Store name not
  confirmed until reserved. Model A puts Apple ID security in the trust chain.
  CloudKit is eventually consistent with quotas (events are idempotent by id).
  iOS limits background execution, so focus completion relies on scheduled
  notifications plus elapsed-time reconciliation.

## Resolved decisions

- OD-4 (2026-11-26): v1 includes a minimal planned day per task, stored in the existing `scheduled_for` column; no calendar time slots.
