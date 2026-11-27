//! `brega backup create` and `brega backup restore` subcommands.
//!
//! Backup is an **outer-encrypted full snapshot** of the vault, as
//! ratified by ADR-018. Materialized domain tables (task titles/notes,
//! habit text, checklist item titles) are plaintext at rest (ADR-014), so
//! the archive itself is sealed with AES-256-GCM under a domain-separated
//! backup key derived from the Vault Key. All framing, crypto, and `SQLite`
//! I/O live in [`brega_storage::backup`]; this module is a thin CLI shell.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use brega_crypto::SecretKey;
use brega_storage::OpenVault;
use brega_storage::backup::{self, RestoreMode};
use clap::Subcommand;

/// Boxed dynamic error alias for command handlers.
type CmdResult = Result<(), Box<dyn std::error::Error>>;

// ── Argument definitions ─────────────────────────────────────────────

/// `brega backup` — create and restore encrypted vault archives.
#[derive(Debug, clap::Args)]
pub struct BackupArgs {
    /// Backup sub-action.
    #[command(subcommand)]
    pub cmd: BackupCmd,
}

/// Backup sub-actions.
#[derive(Debug, Subcommand)]
pub enum BackupCmd {
    /// Create an outer-encrypted full-snapshot backup of the vault.
    Create {
        /// Output archive path (default: `brega-backup-<unix>.bregabak`).
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Restore a vault from an encrypted backup archive.
    ///
    /// Needs only your password and Secret Key (Emergency Kit). An archive
    /// created without the Secret Key is undecryptable by design.
    Restore {
        /// Backup archive to restore from.
        file: PathBuf,
        /// Restore mode — REQUIRED and explicit, never inferred (ADR-018 §3).
        #[arg(long, value_enum)]
        mode: CliRestoreMode,
        /// Allow restoring an archive older than the current vault
        /// (deliberate rollback). Cross-vault / cross-account restores are
        /// never permitted, with or without this flag.
        #[arg(long)]
        force: bool,
    },
}

/// CLI surface for [`RestoreMode`]; kept separate so the storage enum
/// carries no clap dependency.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliRestoreMode {
    /// The original device is gone: restore in place and keep its device
    /// identity, signing key, Lamport clock, sync cursor, and binding.
    DisasterRecovery,
    /// A second/duplicate device: mint a fresh device identity, reset the
    /// sync cursor, and clear the server binding so the clone reconciles
    /// remote history before pushing.
    Clone,
}

impl From<CliRestoreMode> for RestoreMode {
    fn from(mode: CliRestoreMode) -> Self {
        match mode {
            CliRestoreMode::DisasterRecovery => Self::DisasterRecovery,
            CliRestoreMode::Clone => Self::Clone,
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────

/// Handle `brega backup create` against an already-open vault.
pub fn run_create(vault: &OpenVault, out: Option<&Path>) -> CmdResult {
    let out_path = out.map_or_else(default_backup_path, Path::to_path_buf);
    let info = backup::create(vault, &out_path)?;
    println!("Wrote encrypted backup to {}", out_path.display());
    println!("  vault:  {}", info.vault_id);
    println!("  events: {}", info.event_count);
    println!("  size:   {} bytes", info.archive_bytes);
    println!();
    println!(
        "This archive is decryptable ONLY with your password AND Secret Key\n\
         (Emergency Kit). Store the Secret Key separately — a backup without it\n\
         cannot be recovered, by design."
    );
    Ok(())
}

/// Handle `brega backup restore`.
#[allow(clippy::too_many_arguments)]
pub fn run_restore(
    target_vault: &Path,
    file: &Path,
    password: &[u8],
    secret_key: &SecretKey,
    account_id: [u8; 16],
    mode: RestoreMode,
    force: bool,
) -> CmdResult {
    let outcome = backup::restore(
        file,
        target_vault,
        password,
        secret_key,
        account_id,
        mode,
        force,
    )?;
    println!(
        "Restored vault {} from {}",
        outcome.vault_id,
        file.display()
    );
    match outcome.mode {
        RestoreMode::DisasterRecovery => {
            println!(
                "Mode: disaster recovery — kept the original device identity, Lamport\n\
                 clock, sync cursor, and server binding. Resume syncing with `brega sync`."
            );
        }
        RestoreMode::Clone => {
            let dev = outcome
                .new_device_id
                .map_or_else(|| "<unknown>".to_string(), hex16);
            println!("Mode: clone — minted a fresh device identity ({dev}).");
            println!(
                "The server binding was cleared and the sync cursor reset. Reconnect\n\
                 and reconcile remote history BEFORE pushing:\n\
                 \x20 1. brega account login   # re-bind this device to the server\n\
                 \x20 2. brega sync            # pulls + ingests remote history first"
            );
        }
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Default archive path in the current directory, stamped with the
/// current Unix time so repeated backups don't collide.
fn default_backup_path() -> PathBuf {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    PathBuf::from(format!("brega-backup-{secs}.bregabak"))
}

/// Lowercase hex encoding of a 16-byte device id (no external deps).
fn hex16(bytes: [u8; 16]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(32);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
