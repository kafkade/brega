//! Account onboarding bridged to Swift via `UniFFI`.
//!
//! HTTP stays on the Swift edge (`URLSession`); these functions/objects wrap
//! the zero-I/O `brega-account` orchestration. Wire bodies cross the boundary
//! as JSON strings so Swift posts them verbatim to the server the CLI/web also
//! speak to. Secrets (bearer token, channel binding) are hex; the password is
//! never returned or stored.

use std::sync::{Arc, Mutex};

use brega_account::{EmergencyKit, LoginPending, LoginStart, SetupCode, SignupMaterial};
use brega_crypto::SecretKey;

use crate::error::BregaError;

impl From<brega_account::AccountError> for BregaError {
    fn from(e: brega_account::AccountError) -> Self {
        match e {
            brega_account::AccountError::SetupCode => Self::InvalidInput {
                message: "invalid setup code".into(),
            },
            brega_account::AccountError::Auth => Self::InvalidCredentials,
            other => Self::InternalError {
                message: other.to_string(),
            },
        }
    }
}

/// All artifacts produced by signup: the register request body, the printable
/// Emergency Kit, and the scannable Setup Code.
#[derive(uniffi::Record)]
pub struct BregaSignupBundle {
    /// JSON body to POST to `/v1/accounts/register`.
    pub register_request_json: String,
    /// Human-readable Emergency Kit text (also use for the printable sheet).
    pub emergency_kit_text: String,
    /// `TOCK1:` Setup Code string (render as QR on the edge).
    pub setup_code: String,
    /// The account Secret Key (`A4-…`) to surface exactly once.
    pub secret_key: String,
}

/// Decoded Setup Code fields for prefilling a sign-in form.
#[derive(uniffi::Record)]
pub struct BregaSetupCode {
    /// Sign-in server URL.
    pub server_url: String,
    /// Account email / login.
    pub email: String,
    /// Secret Key string (`A4-…`).
    pub secret_key: String,
}

/// Session material after a successful SRP login.
#[derive(uniffi::Record)]
pub struct BregaSessionMaterial {
    /// Bearer token (hex) for `Authorization: Bearer`.
    pub bearer_token: String,
    /// Channel-binding tag (hex) for `X-Brega-Channel-Binding`.
    pub channel_binding: String,
    /// Absolute session expiry (Unix seconds).
    pub expires_at: i64,
}

/// Build signup material for a freshly initialised vault.
///
/// Call after `init_workspace`: pass that vault's `secret_key` kit plus the
/// chosen `username`/`password` and the `server_url`. The vault header is read
/// from the open workspace.
///
/// # Errors
/// Returns [`BregaError`] if the Secret Key is malformed or derivation fails.
#[allow(clippy::needless_pass_by_value)]
#[uniffi::export]
pub fn account_signup_bundle(
    workspace: Arc<crate::Workspace>,
    secret_key: String,
    username: String,
    password: String,
    server_url: String,
) -> Result<BregaSignupBundle, BregaError> {
    let (_aid, sk) = SecretKey::parse(&secret_key).map_err(|_| BregaError::InvalidInput {
        message: "malformed secret key".into(),
    })?;
    workspace.with_header(|header| {
        let material = SignupMaterial::derive(&username, &password, &sk, header, &server_url)?;
        let register_request_json =
            serde_json::to_string(&material.register_request).map_err(|e| {
                BregaError::InternalError {
                    message: e.to_string(),
                }
            })?;
        Ok(BregaSignupBundle {
            register_request_json,
            emergency_kit_text: material.emergency_kit.render_text(),
            setup_code: material.setup_code.encode(),
            secret_key,
        })
    })
}

/// Parse a `TOCK1:` Setup Code into its fields.
///
/// # Errors
/// Returns [`BregaError::InvalidInput`] for a malformed code.
#[uniffi::export]
pub fn parse_setup_code(code: &str) -> Result<BregaSetupCode, BregaError> {
    let c = SetupCode::parse(code)?;
    Ok(BregaSetupCode {
        server_url: c.server_url,
        email: c.email,
        secret_key: c.secret_key,
    })
}

/// Render an Emergency Kit text block from fields (matches signup layout).
#[must_use]
#[uniffi::export]
pub fn emergency_kit_text(server_url: &str, email: &str, secret_key: &str) -> String {
    EmergencyKit {
        email: email.to_string(),
        server_url: server_url.to_string(),
        secret_key: secret_key.to_string(),
    }
    .render_text()
}

/// Begin an SRP login. Returns a handle plus the `srp/start` request JSON.
///
/// # Errors
/// Returns [`BregaError`] if the RNG fails.
#[uniffi::export]
pub fn account_login_start(username: &str) -> Result<Arc<AccountLogin>, BregaError> {
    let (start, req) = LoginStart::new(username)?;
    let json = serde_json::to_string(&req).map_err(|e| BregaError::InternalError {
        message: e.to_string(),
    })?;
    Ok(Arc::new(AccountLogin {
        state: Mutex::new(LoginState::Started(start)),
        start_request_json: json,
    }))
}

enum LoginState {
    Started(LoginStart),
    Pending(LoginPending),
    Spent,
}

/// SRP login state machine handle.
#[derive(uniffi::Object)]
pub struct AccountLogin {
    state: Mutex<LoginState>,
    start_request_json: String,
}

#[allow(clippy::significant_drop_tightening)]
#[uniffi::export]
impl AccountLogin {
    /// The `srp/start` request body to POST.
    #[must_use]
    pub fn start_request_json(&self) -> String {
        self.start_request_json.clone()
    }

    /// Feed the `srp/start` response; returns the `srp/finish` request JSON.
    ///
    /// # Errors
    /// Returns [`BregaError`] for malformed input or bad KDF params.
    pub fn finish(
        &self,
        start_response_json: &str,
        password: &str,
        secret_key: &str,
    ) -> Result<String, BregaError> {
        let (_aid, sk) = SecretKey::parse(secret_key).map_err(|_| BregaError::InvalidInput {
            message: "malformed secret key".into(),
        })?;
        let resp =
            serde_json::from_str(start_response_json).map_err(|e| BregaError::InvalidInput {
                message: e.to_string(),
            })?;
        let mut guard = self.state.lock().map_err(|_| BregaError::InternalError {
            message: "login mutex poisoned".into(),
        })?;
        let LoginState::Started(start) = std::mem::replace(&mut *guard, LoginState::Spent) else {
            return Err(BregaError::InvalidState {
                message: "login already finished".into(),
            });
        };
        let (pending, req) = start.finish(&resp, password, &sk)?;
        *guard = LoginState::Pending(pending);
        serde_json::to_string(&req).map_err(|e| BregaError::InternalError {
            message: e.to_string(),
        })
    }

    /// Verify the `srp/finish` response and return session material.
    ///
    /// # Errors
    /// Returns [`BregaError::InvalidCredentials`] if mutual auth fails.
    pub fn verify(&self, finish_response_json: &str) -> Result<BregaSessionMaterial, BregaError> {
        let resp =
            serde_json::from_str(finish_response_json).map_err(|e| BregaError::InvalidInput {
                message: e.to_string(),
            })?;
        let mut guard = self.state.lock().map_err(|_| BregaError::InternalError {
            message: "login mutex poisoned".into(),
        })?;
        let LoginState::Pending(pending) = std::mem::replace(&mut *guard, LoginState::Spent) else {
            return Err(BregaError::InvalidState {
                message: "login not ready to verify".into(),
            });
        };
        let m = pending.verify(&resp)?;
        Ok(BregaSessionMaterial {
            bearer_token: m.bearer_token,
            channel_binding: m.channel_binding,
            expires_at: m.expires_at,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::init_workspace;
    use tempfile::tempdir;

    #[test]
    fn signup_bundle_and_setup_code_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir
            .path()
            .join("acct.bregavault")
            .to_string_lossy()
            .to_string();
        let init = init_workspace(path, b"pw".to_vec()).unwrap();
        let bundle = account_signup_bundle(
            init.workspace,
            init.secret_key,
            "user@x.test".into(),
            "pw".into(),
            "https://brega.example".into(),
        )
        .unwrap();
        assert!(bundle.register_request_json.contains("srp_verifier"));
        assert!(bundle.emergency_kit_text.contains("NOT stored"));
        let parsed = parse_setup_code(&bundle.setup_code).unwrap();
        assert_eq!(parsed.email, "user@x.test");
        assert_eq!(parsed.server_url, "https://brega.example");
    }

    #[test]
    fn login_start_emits_request() {
        let l = account_login_start("user@x.test").unwrap();
        assert!(l.start_request_json().contains("a_pub"));
    }

    #[test]
    fn bad_setup_code_rejected() {
        assert!(parse_setup_code("nope").is_err());
    }
}
