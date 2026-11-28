# AGENTS.md — guidance for coding agents working on brega

Human contributors: see [CONTRIBUTING.md](CONTRIBUTING.md). This file covers
what an agent needs that isn't obvious from the code.

## Current direction (read first)

The first release is an **iPhone + Mac SwiftUI app over the Rust core**, named
**brega**, syncing through iCloud (CloudKit). On-disk and crypto labels still
say `tock` until vault format v3 (#233).

- Release scope and requirement IDs (R-xx, N-xx): [docs/release-v1.md](docs/release-v1.md).
  It wins over every other doc on scope.
- After v1: [docs/roadmap.md](docs/roadmap.md). Rationale:
  [ADR-021](docs/adr/ADR-021-iphone-mac-first-product-direction.md).
- Work is tracked in the GitHub milestone **v1 — brega iPhone + Mac (personal)**;
  issues cite requirement IDs.
- **Dormant** (keep compiling, don't extend unless asked): `brega-cli` (incl.
  TUI), `brega-server`, `brega-wasm` + `apps/web`, `brega-account` (SRP),
  `brega-caldav`, `brega-import`, `brega-export`, and `apps/ios|macos|watchos`
  (to be replaced by a single `apps/brega` target, #234).

## Layout

| Path | Role |
| --- | --- |
| `crates/brega-core` | Domain model and pure logic (tasks, areas, habits, time, focus, urgency, recurrence, durations, Weekly Review), event and vault types. **No I/O.** |
| `crates/brega-crypto` | AEAD, KDF, signatures, key types. No I/O. |
| `crates/brega-parse` | Date parser and filter DSL. |
| `crates/brega-storage` | SQLite (rusqlite): vault lifecycle, migrations, repositories, event log, sync substrate (`src/sync`), undo, backup. |
| `crates/brega-sync` | Wire format, conflict engine, pairing, recovery. |
| `crates/brega-uniffi` | UniFFI facade for Apple (`Workspace` object, records in `types.rs`, `planning.rs`). |
| `bindings/swift` | `BregaFFI` (generated, gitignored) and `BregaSwift` (hand-written async wrappers + helpers) with XCTest suite. |
| `xtask` | `cargo xtask xcframework` regenerates Swift bindings and builds the XCFramework. |
| `crates/brega-server` | Sync server, **AGPL-3.0** (everything else is Apache-2.0). Don't move code across that boundary without considering the license. |

## Invariants

1. **brega-core has zero I/O** (ADR-001): no filesystem, network, clock reads
   for logic, or platform APIs. Time-zone resolution happens in the platform
   layer and is passed in as instants (see `domain::review::ReviewWeek`).
2. **Encryption is client-side** (ADR-002). Sync payloads are ciphertext; never
   add plaintext to wire frames or server storage.
3. **Schema changes are additive numbered migrations** in
   `crates/brega-storage/src/migrations/` registered in `migrations.rs`. Never
   edit an applied migration (checksummed). Ask before changing the vault header
   format (ADR-013).
4. **Sync is a state diff at sync time** (`brega-storage/src/sync`): domain
   repositories write SQLite directly; `collect_local_changes` diffs rows against
   a per-entity journal. Mixed versions must keep working (R-13): older devices
   apply known columns and carry unknown ones in the journal; upgrades fill new
   columns from it. Adding an **entity kind** is not yet safe for older builds
   (#251). New synced tables go in `sync/registry.rs`.
5. **Timestamps** are written only through `repo::format_timestamp` (fixed-width
   UTC, 9 fractional digits) so text `ORDER BY`/range queries are chronological.
6. **Concurrent same-field edits are not silently overwritten**: they are
   recorded as conflicts (ADR-003); v1 moves to automatic merge with history
   (#241).
7. **Methodology-neutral**: offer views and signals, never enforce a workflow.

## Conventions

- Errors: `thiserror` in library crates, `anyhow` only in binaries. Never put
  key material in errors, logs, or `Debug` output; secret types zeroize.
- Lints: clippy `pedantic` + `nursery` at warn, CI runs with `-D warnings` on
  all targets and features. Tests may `#![allow(clippy::expect_used, ...)]`.
- UniFFI: add new record fields with `#[uniffi(default = …)]` so existing Swift
  initializers keep compiling; expose new `Workspace` methods through
  `BregaSwift/BregaWorkspace.swift` too.
- Tests: focused cases next to the code (`#[cfg(test)]`); cross-device sync
  behavior in `crates/brega-storage/tests/sync_substrate.rs` (two vaults sharing
  a vault key, events shipped by value).
- Docs: Markdown is linted (`.markdownlint-cli2.jsonc`). Record significant
  decisions as ADRs in `docs/adr/` (index in `docs/adr/README.md`).

## Build and test

Toolchain is pinned in `rust-toolchain.toml` (1.95.0, also the MSRV).

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p brega-storage --test sync_substrate        # sync scenarios
cargo build -p brega-core --target wasm32-unknown-unknown --no-default-features --features core
cargo +1.95 check --workspace --all-features              # MSRV job
cargo xtask xcframework && (cd bindings/swift && swift test)   # Apple bindings (macOS)
npx markdownlint-cli2 "**/*.md"
```

## CI and branch protection

Jobs live in `.github/workflows/ci.yml`; the aggregate `CI` job is the required
check. Branch protection is managed by Terraform in `kafkade/github-infra`
(`repos/brega/main.tf`): renaming, adding, or removing gating jobs needs a
matching change there or PRs stay blocked. Flag this whenever you touch CI.

## Git policy

- **Never modify git history**: no `commit`, `push`, `rebase`, `merge`,
  `cherry-pick`, `revert`, `reset`, `tag`, `am`, or `stash drop`, and no branch
  creation or deletion. Applies to all agents and sub-agents, including CI or
  cleanup work.
- Read-only git is fine (`status`, `diff`, `log`, `show`, …), as are `git add`,
  `git mv`, and `git stash push`.
- Present changes and let the maintainer commit.

## Pull requests and changelog

- PR titles use conventional commits, scoped to the main component for
  multi-component changes: `feat(storage): …`, `fix(sync): …`, `docs: …`.
- Use `.github/pull_request_template.md` and fill every section.
- Add user-facing changes to `## [Unreleased]` in `CHANGELOG.md` (Keep a
  Changelog). No entries for refactors, CI, tests, or dependency bumps.

## Releasing

Two phases, driven by the maintainer's machine-global `release` helper (not in
the repo). Agents may only do phase 1's file edits, uncommitted:

1. **Prepare** (release PR): bump `[workspace.package] version` and every
   internal `brega-*` `version = "x.y.z"` in `[workspace.dependencies]` (needed
   by cargo-deny), refresh `Cargo.lock`, stamp `CHANGELOG.md`
   (`[Unreleased]` → `[x.y.z] - <date>`).
2. **Tag** (maintainer only): after merge, `release tag --push` creates `vX.Y.Z`,
   which triggers `.github/workflows/release.yml`.

## Known gotchas

- `getrandom` needs the `js` feature on `wasm32-unknown-unknown`.
- `deny.toml` ignores specific RUSTSEC advisories with reasons; review them
  rather than adding blanket ignores.
- The release workflow uses `--notes-file` so CHANGELOG text isn't shell-parsed.
- `foreign_keys` is not enabled on production connections yet; sync writes
  must still avoid delete-and-reinsert (it would fire `ON DELETE` actions once
  enabled).
