//! Secrets in owlmic.json (identity and pairing keys) are sealed with DPAPI for the signed-in
//! user (SYSTEM_DESIGN section 18.3).

use owlmic_settings::store::Protector;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

pub struct Dpapi;

fn take(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    unsafe {
        let out = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(blob.pbData as _)));
        out
    }
}

impl Protector for Dpapi {
    fn protect(&self, plain: &[u8]) -> Vec<u8> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        // DPAPI only fails without a user profile; an empty value then just fails to unseal later.
        match unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        } {
            Ok(()) => take(out),
            Err(_) => Vec::new(),
        }
    }

    fn unprotect(&self, sealed: &[u8]) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: sealed.len() as u32,
            pbData: sealed.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
            .ok()?
        };
        Some(take(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_secrets_open_again_for_the_same_user() {
        let sealed = Dpapi.protect(b"pairing key");
        assert_ne!(sealed, b"pairing key");
        assert_eq!(
            Dpapi.unprotect(&sealed).as_deref(),
            Some(&b"pairing key"[..])
        );
        assert_eq!(Dpapi.unprotect(b"not sealed"), None);
    }
}
