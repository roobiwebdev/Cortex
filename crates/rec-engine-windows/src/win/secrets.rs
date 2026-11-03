//! SOURCE OF TRUTH KEYWORDS: CredentialStore, Credential Manager, CredWriteW, CredReadW, CredDeleteW, stream key, generic credential, Secrets
//! WHAT:  `rec_core::platform::Secrets` on Windows Credential Manager: one generic
//!        credential per stream URL (`caps::credential_target`), the key as UTF-8 bytes.
//! WHY:   Stream keys never go to the settings file, the webview or logs; Credential
//!        Manager is the per-user secret store Windows provides (the Keychain's
//!        counterpart), visible under Control Panel › Credential Manager › Windows
//!        Credentials as "Cortex stream key:<url>".
//! WHERE: `WinBackend::secrets` (the engine's stream key methods).

use rec_core::platform::Secrets;
use windows::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};
use windows::core::{PCWSTR, PWSTR};

use super::util::{msg, wide};
use crate::caps::credential_target;

pub struct CredentialStore;

impl Secrets for CredentialStore {
    fn get(&self, account: &str) -> Option<String> {
        let target = wide(&credential_target(account));
        let mut p: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `target` is NUL-terminated; on success `p` points at a credential that is
        // read and then freed with CredFree.
        unsafe {
            CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut p).ok()?;
            if p.is_null() {
                return None;
            }
            let c = &*p;
            let out = if c.CredentialBlob.is_null() {
                None
            } else {
                let bytes =
                    std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
                String::from_utf8(bytes.to_vec()).ok()
            };
            CredFree(p as *const _);
            out
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        let mut target = wide(&credential_target(account));
        let mut user = wide("Cortex");
        let mut blob = secret.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: u32::try_from(blob.len()).map_err(|_| "The key is too long")?,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: every pointer in `cred` points into buffers that outlive the call.
        let r = unsafe { CredWriteW(&cred, 0) };
        // Don't leave the key in memory longer than needed.
        blob.fill(0);
        r.map_err(|e| msg("Credential Manager", &e))
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let target = wide(&credential_target(account));
        // SAFETY: `target` is NUL-terminated.
        unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) }
            .map_err(|e| msg("Credential Manager", &e))
    }
}
