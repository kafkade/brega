# ADR-021: iPhone + Mac first product direction

**Status:** Accepted
**Date:** 2026-11-26

> Product-level decision record for the v1 re-baseline. Scope details and
> acceptance criteria live in [release-v1.md](../release-v1.md). Amends the
> platform and release assumptions of [ADR-005](ADR-005-platform-bindings.md),
> [ADR-011](ADR-011-account-based-self-host-two-secret-auth.md), and
> [ADR-014](ADR-014-at-rest-encryption-app-layer-aead.md) as noted below.

## Context

The previous 1.0 target was the CLI plus a self-hostable server and web console,
with Apple apps deferred. The owner now needs the app day to day on an iPhone and
a Mac, sooner than that plan allowed. An assessment (2026-10) also found that
materialized domain tables are stored as plaintext despite documentation saying
otherwise, that sync applies concurrent same-field edits with a review log
rather than a resolution, and that the Apple app sources lack packaging and CI.

## Decision

| ID | Decision |
| --- | --- |
| D-01 | Rename the product **tock → brega**, including on-disk and cryptographic labels, while no real user data exists. |
| D-02 | Primary platforms are **iPhone and macOS**; the CLI moves to the possible roadmap. |
| D-03 | **Rust stays the core**; the app is SwiftUI over **UniFFI**, as one multiplatform target. |
| D-04 | A barebones v1 is acceptable only if later versions extend the vault through additive migrations. |
| D-05 | iPhone ↔ Mac sync in v1 relays encrypted events through the user's **CloudKit private database**. |
| D-06 | v1 covers tasks, focus, time tracking, and habits as one connected loop. |
| D-07 | The vault key lives in **iCloud Keychain**; daily unlock uses Face ID / Touch ID; a printable recovery kit is the fallback. Password + Secret Key (ADR-011) is not used in v1 and can be added later as an extra wrap of the same key. |
| D-08 | Personal distribution first; public App Store later. |
| D-09 | Minimum OS iOS 26 / macOS 26. |
| D-10 | The owner joins the Apple Developer Program. |
| D-11 | Out-of-scope code stays **dormant** in the repository. |
| D-12 | Capacity planning (estimates, area budgets, weekly review) is part of v1; calendar busy time is v1.1 ([ADR-020](ADR-020-external-calendar-busy-time.md)). |
| D-13 | Minimal areas join v1. |
| D-14 | At rest: **SQLCipher** keyed by the vault key plus OS file protection, replacing ADR-014's app-layer-only approach. Concurrent same-field edits merge automatically (latest edit wins, the other value kept in history), replacing ADR-003's review-only handling. Focus sessions can credit a habit. |
| D-15 | Schema migrations for estimate, area budget, and an append-only reschedule log, plus a sync forward-compatibility fix, are approved; the header `format_version` does not change for them. |

## Consequences

**Positive:**

- The release targets the owner's actual devices and daily loop.
- Rust domain, crypto, and sync work is reused; Swift holds only UI and platform glue.
- No server to run for v1; sync stays end-to-end encrypted.
- Encryption at rest becomes real for domain data.

**Negative:**

- Apple ID / iCloud Keychain security becomes part of the trust chain.
- The CLI, server, and web console stop advancing.
- The rename touches every crate, identifier, and format label in one change.

**Neutral:**

- Dormant code can be revived on the same core.
- ADR-011's account model remains valid for a future server path.
