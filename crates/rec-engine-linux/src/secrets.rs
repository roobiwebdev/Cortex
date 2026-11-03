//! SOURCE OF TRUTH KEYWORDS: SecretService, Secrets, stream key, keyring, GNOME Keyring, KWallet
//! WHAT:  `rec_core::platform::Secrets` over the freedesktop Secret Service (GNOME Keyring,
//!        KWallet, KeePassXC), through the `keyring` crate.
//! WHY:   Stream keys never go to the settings file, the webview or logs. Without a running
//!        Secret Service daemon every call fails with a message the UI shows.
//! WHERE: `engine` (set_stream_key, remove_stream_key, with_key_status).

use rec_core::platform::Secrets;

/// The Secret Service "service" attribute for every Cortex secret.
pub const SERVICE: &str = "dev.roobi.cortex.stream-key";

pub struct SecretService;

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())
}

fn explain(e: keyring::Error) -> String {
    match e {
        keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_) => {
            format!("No keyring is available (start GNOME Keyring or KWallet): {e}")
        }
        other => other.to_string(),
    }
}

impl Secrets for SecretService {
    fn get(&self, account: &str) -> Option<String> {
        entry(account).ok()?.get_password().ok()
    }
    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        entry(account)?.set_password(secret).map_err(explain)
    }
    fn delete(&self, account: &str) -> Result<(), String> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(explain(e)),
        }
    }
}
