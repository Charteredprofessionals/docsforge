//! crypto.rs — At-rest data protection and machine-id generation wrapper.
//!
//! Provides DPAPI-backed data encryption on Windows for sensitive settings and offline license files,
//! plus zero-knowledge machine identifier generation for audit and seat binding.

use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::core::error::DocForgeError;

/// Returns a zero-knowledge persistent unique machine identifier for license and audit binding.
///
/// If a machine ID file exists under app data, reads it; otherwise generates a new random UUIDv4,
/// persists it safely to disk, and returns it.
pub fn get_or_create_machine_id() -> Result<String, DocForgeError> {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let id_file = data_dir.join("docforge").join(".machine_id");

    if id_file.exists() {
        let content = fs::read_to_string(&id_file)
            .map_err(|e| DocForgeError::StorageIo(format!("Read machine_id file: {e}")))?;
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    let new_id = format!("mid_{}", Uuid::new_v4());
    if let Some(parent) = id_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| DocForgeError::StorageIo(format!("Create app data dir: {e}")))?;
    }

    fs::write(&id_file, &new_id)
        .map_err(|e| DocForgeError::StorageIo(format!("Write machine_id file: {e}")))?;

    Ok(new_id)
}

/// Encrypts bytes at rest using platform data protection (DPAPI wrapper on Windows).
///
/// **SECURITY WARNING:**
/// - Windows: Uses DPAPI (cryptographically secure)
/// - Other platforms: **FAILS SAFELY** - returns error to prevent insecure storage
///
/// Cross-platform encryption support will be added in v2.0.1 using proper cryptography.
/// The previous XOR obfuscation has been removed as it provided false security.
pub fn encrypt_at_rest(plaintext: &[u8]) -> Result<Vec<u8>, DocForgeError> {
    if plaintext.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(target_os = "windows")]
    {
        win_dpapi_encrypt(plaintext)
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Fail safely: do not pretend to encrypt when we can't
        Err(DocForgeError::StorageIo(
            "Encryption at rest requires Windows DPAPI. \
             This build does not support secure storage. \
             Cross-platform encryption will be available in v2.0.1."
                .to_string(),
        ))
    }
}

/// Decrypts bytes previously encrypted with `encrypt_at_rest`.
///
/// **SECURITY WARNING:**
/// - Windows: Uses DPAPI (cryptographically secure)
/// - Other platforms: **FAILS SAFELY** - returns error to prevent insecure operations
pub fn decrypt_at_rest(ciphertext: &[u8]) -> Result<Vec<u8>, DocForgeError> {
    if ciphertext.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(target_os = "windows")]
    {
        win_dpapi_decrypt(ciphertext)
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Fail safely: do not pretend to decrypt when we can't verify security
        Err(DocForgeError::StorageIo(
            "Decryption at rest requires Windows DPAPI. \
             This build cannot read encrypted data. \
             Cross-platform encryption will be available in v2.0.1."
                .to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
fn win_dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, DocForgeError> {
    use std::ptr::null_mut;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::dpapi::CryptProtectData;
    use winapi::um::wincrypt::DATA_BLOB;

    let mut in_blob = DATA_BLOB {
        cbData: plaintext.len() as DWORD,
        pbData: plaintext.as_ptr() as *mut _,
    };
    let mut out_blob = DATA_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let res = unsafe {
        CryptProtectData(
            &mut in_blob,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut out_blob,
        )
    };

    if res == 0 {
        return Err(DocForgeError::StorageIo(
            "Windows DPAPI CryptProtectData failed".to_string(),
        ));
    }

    let encrypted_bytes = unsafe {
        std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec()
    };

    unsafe {
        winapi::um::winbase::LocalFree(out_blob.pbData as *mut _);
    }

    Ok(encrypted_bytes)
}

#[cfg(target_os = "windows")]
fn win_dpapi_decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, DocForgeError> {
    use std::ptr::null_mut;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::dpapi::CryptUnprotectData;
    use winapi::um::wincrypt::DATA_BLOB;

    let mut in_blob = DATA_BLOB {
        cbData: ciphertext.len() as DWORD,
        pbData: ciphertext.as_ptr() as *mut _,
    };
    let mut out_blob = DATA_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let res = unsafe {
        CryptUnprotectData(
            &mut in_blob,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut out_blob,
        )
    };

    if res == 0 {
        return Err(DocForgeError::StorageIo(
            "Windows DPAPI CryptUnprotectData failed".to_string(),
        ));
    }

    let decrypted_bytes = unsafe {
        std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec()
    };

    unsafe {
        winapi::um::winbase::LocalFree(out_blob.pbData as *mut _);
    }

    Ok(decrypted_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_id_generation() {
        let mid = get_or_create_machine_id().expect("Machine ID must generate");
        assert!(mid.starts_with("mid_"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_at_rest_roundtrip() {
        let secret = b"DOCFORGE_CONFIDENTIAL_KEY_12345";
        let encrypted = encrypt_at_rest(secret).expect("Encryption must succeed on Windows");
        let decrypted = decrypt_at_rest(&encrypted).expect("Decryption must succeed on Windows");
        assert_eq!(&decrypted[..], secret);
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_at_rest_fails_safely_on_non_windows() {
        let secret = b"DOCFORGE_CONFIDENTIAL_KEY_12345";
        let result = encrypt_at_rest(secret);
        assert!(result.is_err(), "Encryption should fail safely on non-Windows");
        
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Windows DPAPI"), "Error should mention Windows requirement");
    }
}
