# Dogfooding brega sync

This guide walks through running your own `brega-server` and syncing two
`brega` CLI vaults through it end to end: initialize a vault, pair a second
device over the real onboarding handshake, and verify a round-trip
(including how conflicts surface). It is the manual companion to the
automated acceptance test in
[`crates/brega-cli/tests/e2e_sync.rs`](../crates/brega-cli/tests/e2e_sync.rs),
which exercises the same end-to-end loop in CI — setting up the second
device with `brega account signup` / `brega account login` (the account flow
from #129) rather than the manual onboarding handshake below.

Everything below is **end-to-end encrypted**: the server only ever stores
opaque ciphertext. Your password and vault key never leave your devices
(see [ADR-002](adr/ADR-002-end-to-end-encryption.md) and the
[encryption note](#how-your-data-stays-private) at the end).

> **Licensing note.** `brega-server` is **AGPL-3.0-only**
> ([ADR-006](adr/ADR-006-licensing-dual-license.md)); the `brega` CLI is
> Apache-2.0. Running your own server is fully supported — the source is in
> [`crates/brega-server/`](../crates/brega-server/).

## Prerequisites

- A built `brega` CLI binary (`cargo build --release -p brega-cli`, then use
  `target/release/brega`, or install from
  [Releases](https://github.com/kafkade/brega/releases)).
- Docker (for the quickest server setup) **or** a Rust toolchain to run the
  server with `cargo`.

Throughout, the CLI reads two environment variables so it can run
non-interactively:

| Variable        | Meaning                                              |
| --------------- | ---------------------------------------------------- |
| `BREGA_VAULT`    | Path to the vault file (created on first use).       |
| `BREGA_PASSWORD` | Vault password (omit it to be prompted interactively). |

## 1. Run the server

### Option A — Docker Compose (recommended)

From the repository root:

```sh
cp .env.example .env
docker compose up -d
```

This builds the stack from [`Dockerfile`](../Dockerfile) +
[`Dockerfile.web`](../Dockerfile.web) and starts `brega-server` behind the
`brega-web` console on port `8080`, persisting the encrypted database in the
`brega-data` named volume. Confirm it is healthy:

```sh
curl -f http://localhost:8080/health
```

For the full self-host walkthrough — first-run admin wizard, TLS, user
management, backup/restore — see the
[self-hosting guide](self-hosting.md).

### Option B — `cargo run`

```sh
cargo run --release -p brega-server -- \
  --bind 127.0.0.1:8080 \
  --data-dir ./brega-server-data
```

The server defaults to **self-hosted** mode, which exposes only the
encrypted blob-store endpoints (no accounts, no billing). Configuration is
also available via environment variables: `BREGA_BIND`, `BREGA_DATA_DIR`, and
`BREGA_MODE`.

For the rest of this guide we assume the server is reachable at
`http://127.0.0.1:8080` — substitute your real host/port (for a remote
server, put it behind TLS).

## 2. Initialize a vault (device A)

Pick a vault path and password, add a task, then push to the server:

```sh
export BREGA_VAULT="$HOME/.local/share/brega/a.bregavault"
export BREGA_PASSWORD="correct horse battery staple"

brega add "Buy groceries #errands"
brega add "Write sync guide"

# Pass --server once; the URL is remembered in the vault thereafter.
brega sync --server http://127.0.0.1:8080
```

The first `brega sync` registers this device with the server and pushes your
events. Expect output like:

```text
Synced with http://127.0.0.1:8080: pushed 2, pulled 0, conflicts 0.
```

## 3. Pair a second CLI (device B)

Device B is a separate vault (think: a second laptop). Pairing transfers
the vault key to it over an authenticated channel, using an out-of-band
**fingerprint check** to defeat a man-in-the-middle. The handshake has two
sides that run concurrently and exchange short hex values, exactly as two
people would read codes to each other.

In **terminal A** (the already-onboarded device), start the invite:

```sh
export BREGA_VAULT="$HOME/.local/share/brega/a.bregavault"
export BREGA_PASSWORD="correct horse battery staple"

brega onboard invite --server http://127.0.0.1:8080
```

It prints values to hand to the new device and then waits:

```text
Share these values with the new device's `brega onboard accept`:
  --server http://127.0.0.1:8080
  --vault-id <hex>
  --inviter-pubkey <hex>
  --inviter-fingerprint <hex>

Acceptor public key (hex):
```

In **terminal B** (the new device, a fresh vault path and its own
password), run `accept` with the four values above:

```sh
export BREGA_VAULT="$HOME/.local/share/brega/b.bregavault"
export BREGA_PASSWORD="a different password for device B"

brega onboard accept \
  --server http://127.0.0.1:8080 \
  --vault-id <hex> \
  --inviter-pubkey <hex> \
  --inviter-fingerprint <hex>
```

`accept` first verifies the inviter's public key against the
`--inviter-fingerprint` you supplied (aborting on mismatch — this is the
anti-MITM check). It then prints **its own** three values and starts
polling:

```text
Give these values to the inviter's `brega onboard invite` prompt:
  Acceptor public key: <hex>
  Acceptor fingerprint: <hex>
  Acceptor device id: <hex>

Waiting for the inviter to upload the vault key...
```

Back in **terminal A**, type the three acceptor values into the invite
prompts in the order requested (public key, fingerprint, device id). The
inviter wraps the vault key for device B and uploads it:

```text
Vault key blob uploaded. The new device can now finish onboarding.
```

Within a couple of seconds, **terminal B** finishes:

```text
Onboarding complete. Vault created at <path>.
```

Device B now holds the same vault key and has pulled the existing history.
Confirm it sees device A's tasks:

```sh
brega ls          # device B should list "Buy groceries" and "Write sync guide"
```

## 4. Verify a round-trip

With both devices paired, prove changes propagate both ways. Device B adds
a task and pushes; device A pulls it:

```sh
# Device B (its env still set)
brega add "Pulled from B"
brega sync                 # --server is remembered; no flag needed

# Device A
brega sync
brega ls                   # now includes "Pulled from B"
```

Modifications and completions propagate the same way:

```sh
# Device A completes a task and pushes
brega done 1
brega sync

# Device B pulls and sees it done
brega sync
brega ls
```

After a sync round in each direction, both vaults converge to the same set
of tasks and statuses.

## 5. Review conflicts

Sync never silently overwrites concurrent edits
([ADR-003](adr/ADR-003-event-sourced-sync.md)). If two devices change the
**same field** of the **same task** before syncing, the change surfaces as
a conflict for you to review.

```sh
# On A and B, edit the same task's title differently *without* syncing:
# Device A
brega modify 1 title:RenamedByA
brega sync

# Device B
brega modify 1 title:RenamedByB
brega sync
```

Device B's sync reports the conflict:

```text
Synced with http://127.0.0.1:8080: pushed 1, pulled 1, conflicts 1.
Review conflicts with `brega sync conflicts`.
```

List and resolve it:

```sh
brega sync conflicts
# Unresolved conflicts:
#   [<id>] task <entity> — <detail>
# Resolve with `brega sync resolve <id>`.

brega sync resolve <id>
# Resolved conflict <id>.

brega sync conflicts
# No unresolved conflicts.
```

## 6. Pair the Apple app (coming soon)

> **Status: not yet shipped.** The `brega-uniffi` crate provides the
> Rust↔Swift bindings, but the SwiftUI app for iOS / iPadOS / macOS /
> watchOS is still in progress. This section describes the intended flow so
> you know what to expect.

When the app ships, pairing a phone will mirror the CLI handshake above:
the existing device generates an invite (shown as a QR code instead of hex
strings), the app scans it, both sides display a short fingerprint for you
to compare out of band, and on confirmation the vault key is wrapped and
transferred through the same server endpoints. No plaintext ever touches
the server — the phone derives nothing from your password beyond what the
CLI already does.

## How your data stays private

The server is a dumb encrypted blob store. It sees only:

- **Event payloads** — each one is AEAD-encrypted on the device before
  upload; the server stores the ciphertext in `server_events.payload` and
  never holds a key to open it.
- **Onboarding blobs** — the vault key wrapped *to the new device's public
  key*; only that device can unwrap it.

It never receives your password, your vault key, or any task text. The
automated acceptance test asserts exactly this: after a full multi-device
run it scans every stored blob and fails if any plaintext task title
appears. See [ADR-002](adr/ADR-002-end-to-end-encryption.md) for the key
hierarchy and [ADR-006](adr/ADR-006-licensing-dual-license.md) for why the
server is AGPL-licensed.
