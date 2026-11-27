# Security Policy

## Audit status: unaudited (pre-audit)

> **brega's own cryptographic protocol and implementation have not yet been
> independently audited.** As of 1.0, only the underlying
> [RustCrypto](https://github.com/RustCrypto) *primitives* brega builds on have
> had external review — brega's **composition** of them (key hierarchy, 2SKD,
> vault format & AEAD usage, the SRP-6a handshake and channel binding, the sync
> protocol, and the server's zero-knowledge claims) has **not** been reviewed by
> an external party.

This is a deliberate, documented decision for 1.0, not an oversight — see
[ADR-015](docs/adr/ADR-015-pre-1.0-security-audit-status.md). Most real-world
cryptographic failures are composition and protocol errors rather than broken
primitives, so "built on audited crates" is **not** the same as "audited
product." We are being explicit about that distinction so you can make an
informed choice.

**What this means for your threat model.** brega's marketed guarantees
(end-to-end encryption, zero-knowledge sync server, two-secret key derivation)
are designed in good faith and documented in the ADRs and the
[threat model](docs/architecture.md#55-threat-model), but they have not been
independently verified. If your use case requires an externally attested
cryptographic guarantee, treat brega as **pre-audit** and weigh that accordingly.

**Our commitment.** We have scoped an external crypto/security review — the full
surface list, artifacts, and findings process are published in the
[audit scoping brief](docs/security/audit-scope.md) — and we intend to commission
it, track and remediate findings, and publish a summary/attestation. This
"unaudited (pre-audit)" notice will be **updated when that review lands**, not
quietly removed beforehand.

If you have the expertise to review any of the surfaces in the scoping brief, we
welcome it — please coordinate via the private reporting channel below.

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 1.x     | :white_check_mark: |

Only the latest release is supported with security updates.

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

If you discover a security vulnerability in brega, please report it through
GitHub's private vulnerability reporting feature:

1. Go to the [Security tab](https://github.com/kafkade/brega/security) of this repository
2. Click **"Report a vulnerability"**
3. Fill out the form with details about the vulnerability

### What to expect

- **Acknowledgment**: We will acknowledge your report within **48 hours**
- **Assessment**: We will assess the severity and impact within **7 days**
- **Resolution**: We aim to release a fix within **90 days** of the initial report, depending on complexity
- **Disclosure**: We will coordinate disclosure timing with you

### What to include

- A description of the vulnerability
- Steps to reproduce the issue
- The potential impact
- Any suggested fixes (if you have them)

### Scope

Security issues in brega may include:

- Cryptographic weaknesses in the vault format or key hierarchy
- Key material exposure in error messages, logs, or Debug output
- Vault key leakage through session files or memory
- Bypass of encryption (plaintext data reaching the server/sync layer)
- Argon2id parameter downgrade attacks
- SRP-6a authentication bypass or verifier leakage
- SQLite injection in the storage layer
- Path traversal in import/export file handling
- Cross-domain data leakage (task data visible in habit context without authorization)
- Memory safety issues (though `unsafe` code is forbidden in core)

### Out of scope

- Third-party forks or unofficial builds
- Issues in upstream dependencies (report those to the dependency maintainers)
- Social engineering attacks (phishing for the master password)
- Physical access to an unlocked device

## Cryptographic Details

brega defines **no custom cryptographic primitives**. It uses audited
[RustCrypto](https://github.com/RustCrypto) crates exclusively:

- **AES-256-GCM** for symmetric encryption (aes-gcm crate)
- **Argon2id** for password-based key derivation (argon2 crate)
- **HKDF-SHA256** for domain-separated key derivation (hkdf crate)
- **SRP-6a** for zero-knowledge authentication (srp crate)
- **X25519** for key exchange (x25519-dalek crate)

All key types implement `Zeroize` and `ZeroizeOnDrop` for memory safety.

> **Note:** These *primitives* are independently audited; brega's **protocol and
> its composition of them are not yet** (see
> [Audit status](#audit-status-unaudited-pre-audit) above). Using audited
> building blocks does not by itself guarantee the assembly is correct.

## Encryption at Rest

Current builds protect data at rest with **application-layer AEAD** only, not
full-database (SQLCipher) encryption (see
[ADR-014](docs/adr/ADR-014-at-rest-encryption-app-layer-aead.md), now superseded
by [ADR-021](docs/adr/ADR-021-iphone-mac-first-product-direction.md)). The on-disk
format is marked `storage_layout = "sqlite-plain-app-aead-v0"`.

The local vault is a **plain SQLite** file. Confidential data is encrypted with
AES-256-GCM under a key derived from the Vault Key (VK) **before** it is written:

- **Event payloads** (task, habit, time-block, and focus contents, notes, etc.)
  are sealed with per-item keys and domain-separated AAD.
- **Per-device signing keys** are AEAD-wrapped under a VK-derived key.
- **VK itself is never stored in the clear** — it is wrapped under the MEK, which
  is derived from the two-secret Unlock Root Key (password **and** 128-bit Secret
  Key, Argon2id-hardened; see
  [ADR-011](docs/adr/ADR-011-account-based-self-host-two-secret-auth.md)).

**What at-rest AEAD protects today:** the event-log payloads (the copies of your
data that sync) and the device signing keys. VK cannot be recovered from a stolen
file without the password **and** the Secret Key.

**What it does *not* protect — current builds:** the materialized domain tables
that the app reads from are **plaintext**. A stolen vault file exposes task
titles and notes, habit text, checklist items, time-block titles, and the undo
log, in addition to structure and metadata (schema, timestamps, entity kinds and
UUIDs, device IDs, ciphertext sizes). Until this is fixed, rely on OS full-disk
encryption (FileVault, LUKS, or BitLocker) and do not store sensitive data in
current builds.

**Planned fix:** the first release encrypts the whole database with **SQLCipher**
keyed by VK, plus OS file protection
([ADR-021](docs/adr/ADR-021-iphone-mac-first-product-direction.md) D-14,
[release-v1.md](docs/release-v1.md) N-02).

Note that brega's protection against a **hostile sync host** does not depend on
at-rest storage encryption: sync events carry their own AEAD envelope, so the
server only ever sees opaque ciphertext regardless of the local storage format.

## Thank You

We appreciate responsible disclosure and will credit reporters (with their
permission) in our release notes and changelog.
