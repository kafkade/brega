# brega

> Unified personal productivity engine — tasks, habits, time tracking,
> and focus timer fused into a single end-to-end encrypted, local-first
> system.

## Security status: unaudited (pre-audit)

brega's cryptography is built on audited
[RustCrypto](https://github.com/RustCrypto) primitives, but brega's **own**
protocol and implementation (key hierarchy, 2SKD, vault format, SRP-6a handshake,
sync protocol, server zero-knowledge claims) have **not yet** been independently
reviewed. This is a deliberate, documented 1.0 decision — see
[SECURITY.md](SECURITY.md) and
[ADR-015](docs/adr/ADR-015-pre-1.0-security-audit-status.md). Weigh this before
trusting brega with high-sensitivity data.

## Platforms

> **Direction change (2026-11-26).** The first release is an **iPhone + Mac app**
> (SwiftUI over the Rust core), renamed **brega** (formerly tock), with iCloud sync. The CLI,
> self-hosted server, and web console below remain in the repository but are
> **dormant** and not part of that release. Scope: [docs/release-v1.md](docs/release-v1.md);
> later plans: [docs/roadmap.md](docs/roadmap.md); rationale:
> [ADR-021](docs/adr/ADR-021-iphone-mac-first-product-direction.md).
>
> **At-rest warning:** current builds store task and habit text in plaintext
> inside the vault file; see [SECURITY.md](SECURITY.md#encryption-at-rest).

The rest of this README describes the existing CLI, which is now dormant.

## Features

### Task management

```sh
brega add "Buy groceries #errands !H due:tomorrow"
brega ls                          # list tasks sorted by urgency
brega show 1                      # task detail view
brega mod 1 !M due:friday         # modify priority + deadline
brega done 1 2 3                  # batch complete
brega view today                  # built-in smart views
brega urgency 1                   # explain urgency breakdown
```

Sigil syntax for inline metadata: `#tag`, `!H/M/L` (priority),
`due:tomorrow` (natural language dates). Filter with `status:pending`,
`tag:work`, `+TODAY`, `+OVERDUE`. Six built-in views: inbox, today,
upcoming, anytime, someday, logbook.

### Time tracking

```sh
brega time start "Review PR #code"   # auto-creates task + starts timer
brega time start 1                   # start on existing task
brega time stop                      # stop current block
brega time resume                    # re-start most recent
brega time blocks today              # list blocks
brega time report week               # aggregated report
brega time edit 1 --title "Updated"  # edit a block
```

### Focus timer (Pomodoro)

```sh
brega focus start --task 1 --cycles 4 --work 25
brega focus done              # complete a cycle → auto-logs time block
brega focus skip-break        # skip break, start working
brega focus pause / resume / stop
brega focus status            # show active session
brega focus stats today       # completed cycles, focus time
brega focus history 1         # per-task focus history
```

### Habit tracking

```sh
brega habit add "Read 10 pages" --identity "I am a reader" --cue "After dinner" --daily
brega habit add "No social media" --direction break
brega habit log 1                 # log completion
brega habit slip 2                # log a break-habit slip
brega habit skip 1 --reason flu   # skip without breaking streak
brega habit status                # all habits with streaks + levels
brega habit streaks 1             # streak history
brega habit remind 1 --at 07:00   # set reminder
```

Identity statements, habit stacking (`--stack-after`), cadences (daily,
N×/week, specific days), Fibonacci leveling (Spark → Embodied), and
grace days (skip/freeze).

### Task dependencies

```sh
brega depend 1 on 2               # task 1 blocked until task 2 is done
brega undepend 1 from 2           # remove dependency
brega ls +BLOCKED                 # list blocked tasks
brega ls +BLOCKING                # list tasks blocking others
brega show 1                      # shows dependencies + dependents
```

Circular dependency detection walks the chain (max depth 100). Blocked
tasks receive an urgency penalty so they sink in listings.

### Recurring tasks

```sh
brega add "Pay rent" --recur monthly
brega add "Standup" --recur daily
brega add "Review" --recur every-2w
brega done 1                      # auto-creates next instance
brega show 1                      # shows recurrence details
```

Supports `daily`, `weekly`, `monthly`, `yearly`, `every-Nd`, `every-Nw`.
Periodic mode anchors next instance to the due date; chained mode anchors
to the completion date.

### Named contexts

```sh
brega context define work "tag:work status:pending"
brega context set work            # all listings now filter by work
brega ls                          # shows [ctx: work] prefix
brega context clear               # remove active context
brega context ls                  # list all contexts (* = active)
brega context rm work
```

Contexts are named saved filters that are automatically AND-ed into
every `brega ls`, `brega view`, and `brega report show` command.

### Interactive TUI

```sh
brega tui
```

Three-pane ratatui terminal interface: sidebar (views + projects), task
list (sorted by urgency), and task detail. Vim-style keys: `j`/`k` to
navigate, `Tab` to switch panes, `Enter` to select, `d` to complete,
`x` to delete, `r` to refresh, `q` to quit.

### More

- **Projects & areas** — `brega project add/ls/archive`, `brega area add/ls`
- **Tags** — `#tag` sigils, `brega tag ls/rename`
- **Urgency scoring** — `brega urgency 1` for component breakdown
- **Custom reports** — `brega report define overdue --query '+OVERDUE' --sort deadline`
- **User-defined attributes** — `brega uda add effort --type number`, filter with `uda.effort:5`
- **Hook scripts** — external scripts at `~/.config/brega/hooks/` for lifecycle events
- **Output formats** — `--format table|compact|json`, per-command `--json`
- **Shell completions** — `brega completions bash|zsh|fish|elvish|powershell`
- **JSON import/export** — `brega export json`, `brega import json --file backup.json`
- **Encrypted vault** — password-protected, AES-256-GCM per-event AEAD, Ed25519 signed event log
- **Multi-device sync** — end-to-end encrypted sync through a self-hostable server; see the [dogfooding guide](docs/dogfooding.md)
- **Self-hosting** — one-command Docker Compose stack with a web **admin + onboarding console** (first-run wizard, account self-service) and automatic TLS; the browser is not a full productivity client for 1.0 (tasks/habits/time live in the CLI; the Apple apps arrive in 1.x). See the [self-hosting guide](docs/self-hosting.md)
- **No lock-in** — `brega account export` downloads everything your server holds, and `brega account adopt --migrate` moves your account to a different server with your crypto identity intact; see the [migration guide](docs/migration.md)

## Install

### Homebrew (macOS / Linux)

```sh
brew install kafkade/tap/brega
```

This pulls the signed, notarized release binary from the
[`kafkade/homebrew-tap`](https://github.com/kafkade/homebrew-tap) tap, which is
published automatically on each tagged release.

### Prebuilt binaries

Download the latest release from
[GitHub Releases](https://github.com/kafkade/brega/releases) (Linux, macOS, and
Windows; shell + PowerShell installers).

### From source

```sh
git clone https://github.com/kafkade/brega.git
cd brega
cargo build --release -p brega-cli
# Binary at target/release/brega (or brega.exe on Windows)
```

## Repository layout

```text
brega/
├── Cargo.toml                  # workspace
├── rust-toolchain.toml         # pinned 1.88.0
├── deny.toml                   # cargo-deny config
├── dist-workspace.toml         # cargo-dist config
├── flake.nix                   # Nix dev shell
├── crates/
│   ├── brega-core/              # PURE: domain model, urgency, crypto types
│   ├── brega-crypto/            # PURE: AES-256-GCM, Argon2id, HKDF, Ed25519
│   ├── brega-parse/             # PURE: filter DSL + natural-language dates
│   ├── brega-storage/           # SQLite vault, repos, migrations
│   ├── brega-cli/               # `brega` binary (clap CLI + ratatui TUI)
│   ├── brega-import/            # JSON importer
│   ├── brega-export/            # JSON exporter
│   ├── brega-sync/              # event log, sync protocol (foundation)
│   ├── brega-server/            # Axum sync server — AGPL-3.0-only
│   └── brega-uniffi/            # UniFFI bindings (Apple apps ship in 1.x)
└── docs/
    ├── architecture.md
    ├── dogfooding.md           # self-host + multi-device sync guide
    ├── migration.md            # move servers + download your data
    ├── adr/                    # 15 Architecture Decision Records
    └── distribution/
```

## Licensing

Dual-licensed per [ADR-006](docs/adr/ADR-006-licensing-dual-license.md):

- All crates **except** `brega-server` — [Apache-2.0](LICENSE-APACHE).
- `brega-server` — [AGPL-3.0-only](LICENSE-AGPL).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All commits require a DCO
sign-off (`git commit -s`).

## Development

```sh
# Build + test
cargo build --workspace
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo deny check

# WASM smoke build (CI gate)
cargo build -p brega-core --target wasm32-unknown-unknown --no-default-features --features core
```

Nix users: `nix develop` drops you into a shell with the pinned toolchain
and all auxiliary tools.
