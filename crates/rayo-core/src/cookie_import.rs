//! Import cookies from real browser profiles.
//!
//! Reads Chromium-based browser cookie databases (SQLite), decrypts values
//! using the OS keychain (macOS) or default password (Linux), and converts
//! them to [`SetCookie`] for injection into the headless browser.
//!
//! Supported browsers: Chrome, Arc, Brave, Edge, Chromium.

use std::fmt;
use std::path::PathBuf;

use crate::cookie::{SameSite, SetCookie};
use crate::error::RayoError;

/// Supported browser types for cookie import.
#[derive(Debug, Clone, Copy)]
pub enum BrowserType {
    Chrome,
    Arc,
    Brave,
    Edge,
    Chromium,
}

impl fmt::Display for BrowserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chrome => write!(f, "Chrome"),
            Self::Arc => write!(f, "Arc"),
            Self::Brave => write!(f, "Brave"),
            Self::Edge => write!(f, "Edge"),
            Self::Chromium => write!(f, "Chromium"),
        }
    }
}

impl BrowserType {
    /// Parse from a string (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "chrome" => Some(Self::Chrome),
            "arc" => Some(Self::Arc),
            "brave" => Some(Self::Brave),
            "edge" => Some(Self::Edge),
            "chromium" => Some(Self::Chromium),
            _ => None,
        }
    }

    /// macOS Keychain service name for the browser's cookie encryption key.
    #[cfg(target_os = "macos")]
    fn keychain_service(&self) -> &'static str {
        match self {
            Self::Chrome => "Chrome Safe Storage",
            Self::Arc => "Arc Safe Storage",
            Self::Brave => "Brave Safe Storage",
            Self::Edge => "Microsoft Edge Safe Storage",
            Self::Chromium => "Chromium Safe Storage",
        }
    }

    /// Base directory for browser profiles.
    fn profile_base_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();

        #[cfg(target_os = "macos")]
        {
            let app_support = format!("{home}/Library/Application Support");
            match self {
                Self::Chrome => PathBuf::from(format!("{app_support}/Google/Chrome")),
                Self::Arc => PathBuf::from(format!("{app_support}/Arc/User Data")),
                Self::Brave => PathBuf::from(format!("{app_support}/BraveSoftware/Brave-Browser")),
                Self::Edge => PathBuf::from(format!("{app_support}/Microsoft Edge")),
                Self::Chromium => PathBuf::from(format!("{app_support}/Chromium")),
            }
        }

        #[cfg(target_os = "linux")]
        {
            let config = format!("{home}/.config");
            match self {
                Self::Chrome => PathBuf::from(format!("{config}/google-chrome")),
                Self::Arc => PathBuf::from(format!("{config}/arc")),
                Self::Brave => PathBuf::from(format!("{config}/BraveSoftware/Brave-Browser")),
                Self::Edge => PathBuf::from(format!("{config}/microsoft-edge")),
                Self::Chromium => PathBuf::from(format!("{config}/chromium")),
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = home;
            PathBuf::new()
        }
    }

    /// Cookie database path for a given profile.
    fn cookie_db_path(&self, profile: &str) -> PathBuf {
        self.profile_base_dir().join(profile).join("Cookies")
    }

    /// List available profiles that have cookie databases.
    pub fn list_profiles(&self) -> Vec<String> {
        let base = self.profile_base_dir();
        let mut profiles = Vec::new();

        if base.join("Default").join("Cookies").exists() {
            profiles.push("Default".to_string());
        }

        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("Profile ") && entry.path().join("Cookies").exists() {
                    profiles.push(name);
                }
            }
        }

        profiles
    }
}

/// All browser types for enumeration.
const ALL_BROWSERS: [BrowserType; 5] = [
    BrowserType::Chrome,
    BrowserType::Arc,
    BrowserType::Brave,
    BrowserType::Edge,
    BrowserType::Chromium,
];

/// List browsers that have cookie databases on this machine.
pub fn list_available_browsers() -> Vec<(BrowserType, Vec<String>)> {
    ALL_BROWSERS
        .into_iter()
        .filter_map(|b| {
            let profiles = b.list_profiles();
            if profiles.is_empty() {
                None
            } else {
                Some((b, profiles))
            }
        })
        .collect()
}

/// Import cookies from a real browser's cookie database.
///
/// Reads the Chromium SQLite cookie store, decrypts encrypted values,
/// and returns `SetCookie` objects ready to inject into the headless browser.
pub fn import_cookies(
    browser: BrowserType,
    domain: Option<&str>,
    profile: Option<&str>,
) -> Result<Vec<SetCookie>, RayoError> {
    let profile = profile.unwrap_or("Default");
    let db_path = browser.cookie_db_path(profile);

    if !db_path.exists() {
        return Err(RayoError::CookieError(format!(
            "Cookie database not found: {}. Is {} installed? Available profiles: {:?}",
            db_path.display(),
            browser,
            browser.list_profiles(),
        )));
    }

    let key = derive_decryption_key(browser)?;
    read_cookies_from_db(&db_path, domain, &key)
}

/// Derive the AES-128 key used to decrypt Chrome cookie values.
///
/// Uses PBKDF2-HMAC-SHA1 with the browser's encryption password,
/// salt "saltysalt", 1003 iterations, producing a 16-byte key.
fn derive_decryption_key(browser: BrowserType) -> Result<[u8; 16], RayoError> {
    let password = get_encryption_password(browser)?;

    let mut key = [0u8; 16];
    pbkdf2::pbkdf2::<hmac::Hmac<sha1::Sha1>>(password.as_bytes(), b"saltysalt", 1003, &mut key)
        .map_err(|e| RayoError::CookieError(format!("PBKDF2 key derivation failed: {e}")))?;

    Ok(key)
}

/// Get the encryption password from the macOS Keychain.
#[cfg(target_os = "macos")]
fn get_encryption_password(browser: BrowserType) -> Result<String, RayoError> {
    let service = browser.keychain_service();
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", service, "-w"])
        .output()
        .map_err(|e| RayoError::CookieError(format!("Failed to run `security` command: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RayoError::CookieError(format!(
            "Failed to get encryption key from Keychain for '{}': {}. \
             You may need to allow access in the Keychain prompt.",
            service,
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// On Linux, Chromium uses "peanuts" as the default encryption password.
#[cfg(target_os = "linux")]
fn get_encryption_password(_browser: BrowserType) -> Result<String, RayoError> {
    Ok("peanuts".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_encryption_password(_browser: BrowserType) -> Result<String, RayoError> {
    Err(RayoError::CookieError(
        "Cookie import is only supported on macOS and Linux".to_string(),
    ))
}

/// Raw cookie row from the Chromium SQLite database.
struct RawCookie {
    host_key: String,
    name: String,
    value: String,
    encrypted_value: Vec<u8>,
    path: String,
    is_secure: bool,
    is_httponly: bool,
    expires_utc: i64,
    samesite: i32,
}

/// Read and decrypt cookies from the Chromium SQLite cookie database.
fn read_cookies_from_db(
    db_path: &std::path::Path,
    domain: Option<&str>,
    key: &[u8; 16],
) -> Result<Vec<SetCookie>, RayoError> {
    // Copy database files to a temp dir to avoid lock contention with the running browser.
    // Chrome uses WAL mode, so we copy the -wal and -shm files too.
    let temp_dir = tempfile::tempdir()
        .map_err(|e| RayoError::CookieError(format!("Failed to create temp dir: {e}")))?;
    let temp_db = temp_dir.path().join("Cookies");

    std::fs::copy(db_path, &temp_db).map_err(|e| {
        RayoError::CookieError(format!(
            "Failed to copy cookie database: {e}. Is the browser running with a lock?"
        ))
    })?;

    // Copy WAL/SHM files if present (Chrome names them Cookies-wal, Cookies-shm)
    if let Some(parent) = db_path.parent() {
        for suffix in ["Cookies-wal", "Cookies-shm"] {
            let src = parent.join(suffix);
            if src.exists() {
                let _ = std::fs::copy(&src, temp_dir.path().join(suffix));
            }
        }
    }

    let conn = rusqlite::Connection::open_with_flags(
        &temp_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| RayoError::CookieError(format!("Failed to open cookie database: {e}")))?;

    // Read the cookie database version from the meta table.
    // Version 24+ prepends a 32-byte SHA256 hash to the encrypted value.
    let db_version: u32 = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let base_query = "SELECT host_key, name, value, encrypted_value, path, \
                      is_secure, is_httponly, expires_utc, samesite FROM cookies";

    let mut cookies = Vec::new();

    if let Some(domain) = domain {
        let query = format!("{base_query} WHERE host_key LIKE ?1");
        let pattern = format!("%{domain}%");
        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| RayoError::CookieError(format!("SQL prepare failed: {e}")))?;
        let rows = stmt
            .query_map([&pattern], map_cookie_row)
            .map_err(|e| RayoError::CookieError(format!("SQL query failed: {e}")))?;
        for row in rows {
            if let Some(cookie) = process_row(row, key, db_version)? {
                cookies.push(cookie);
            }
        }
    } else {
        let mut stmt = conn
            .prepare(base_query)
            .map_err(|e| RayoError::CookieError(format!("SQL prepare failed: {e}")))?;
        let rows = stmt
            .query_map([], map_cookie_row)
            .map_err(|e| RayoError::CookieError(format!("SQL query failed: {e}")))?;
        for row in rows {
            if let Some(cookie) = process_row(row, key, db_version)? {
                cookies.push(cookie);
            }
        }
    }

    Ok(cookies)
}

fn map_cookie_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawCookie> {
    Ok(RawCookie {
        host_key: row.get(0)?,
        name: row.get(1)?,
        value: row.get(2)?,
        encrypted_value: row.get(3)?,
        path: row.get(4)?,
        is_secure: row.get::<_, i32>(5)? != 0,
        is_httponly: row.get::<_, i32>(6)? != 0,
        expires_utc: row.get(7)?,
        samesite: row.get(8)?,
    })
}

/// Process a raw cookie row: decrypt value and convert to SetCookie.
fn process_row(
    row: rusqlite::Result<RawCookie>,
    key: &[u8; 16],
    db_version: u32,
) -> Result<Option<SetCookie>, RayoError> {
    let raw = row.map_err(|e| RayoError::CookieError(format!("Failed to read cookie row: {e}")))?;

    // Prefer encrypted_value; fall back to plaintext value column
    let value = if !raw.encrypted_value.is_empty() {
        match decrypt_cookie_value(&raw.encrypted_value, key, db_version) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(
                    "Failed to decrypt cookie '{}': {e} (enc_len={}, prefix={:?})",
                    raw.name,
                    raw.encrypted_value.len(),
                    raw.encrypted_value.get(..3)
                );
                raw.value
            }
        }
    } else {
        raw.value
    };

    if value.is_empty() {
        return Ok(None);
    }

    // Chrome epoch: microseconds since 1601-01-01
    // Unix epoch: seconds since 1970-01-01
    // Delta: 11,644,473,600 seconds
    let expires = if raw.expires_utc > 0 {
        Some((raw.expires_utc as f64 / 1_000_000.0) - 11_644_473_600.0)
    } else {
        None
    };

    let same_site = match raw.samesite {
        2 => Some(SameSite::Strict),
        1 => Some(SameSite::Lax),
        0 => Some(SameSite::None),
        _ => None,
    };

    Ok(Some(SetCookie {
        name: raw.name,
        value,
        domain: Some(raw.host_key),
        path: Some(raw.path),
        url: None,
        secure: Some(raw.is_secure),
        http_only: Some(raw.is_httponly),
        same_site,
        expires,
    }))
}

/// Decrypt a Chromium-encrypted cookie value.
///
/// Chromium encrypts cookie values with a "v10" prefix followed by
/// AES-128-CBC ciphertext (IV = 0x00*16) with PKCS7 padding.
fn decrypt_cookie_value(encrypted: &[u8], key: &[u8; 16], db_version: u32) -> Result<String, RayoError> {
    if encrypted.len() < 3 {
        return String::from_utf8(encrypted.to_vec())
            .map_err(|e| RayoError::CookieError(format!("Cookie value is not valid UTF-8: {e}")));
    }

    match &encrypted[..3] {
        b"v10" => {
            let ciphertext = &encrypted[3..];
            if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
                return Err(RayoError::CookieError(format!(
                    "Invalid ciphertext length: {}",
                    ciphertext.len()
                )));
            }
            let mut plaintext = decrypt_aes_128_cbc(ciphertext, key);

            // Cookie DB version 24+ prepends a 32-byte SHA256 hash of the
            // cookie's domain to the encrypted value. Strip it.
            // https://github.com/nicholasxjy/pycookiecheat/blob/main/src/pycookiecheat/chrome.py#L92-L96
            if db_version >= 24 && plaintext.len() > 32 {
                plaintext = plaintext[32..].to_vec();
            }

            String::from_utf8(plaintext).map_err(|e| {
                RayoError::CookieError(format!("Decrypted cookie is not valid UTF-8: {e}"))
            })
        }
        _ => {
            // Unknown prefix — try as raw UTF-8
            String::from_utf8(encrypted.to_vec()).map_err(|e| {
                RayoError::CookieError(format!("Cookie value is not valid UTF-8: {e}"))
            })
        }
    }
}

/// AES-128-CBC decryption with PKCS7 unpadding.
fn decrypt_aes_128_cbc(ciphertext: &[u8], key: &[u8; 16]) -> Vec<u8> {
    use aes::Aes128;
    use aes::cipher::generic_array::GenericArray;
    use aes::cipher::{BlockDecrypt, KeyInit};

    let cipher = Aes128::new(GenericArray::from_slice(key));
    // Chromium uses space (0x20) as IV, not zero
    let iv = [0x20u8; 16];
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    let mut prev = iv;

    for chunk in ciphertext.chunks_exact(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        for (i, byte) in block.iter().enumerate() {
            plaintext.push(byte ^ prev[i]);
        }
        prev.copy_from_slice(chunk);
    }

    // Remove PKCS7 padding
    if let Some(&pad_len) = plaintext.last() {
        let n = pad_len as usize;
        if n > 0 && n <= 16 && plaintext.len() >= n {
            let valid = plaintext[plaintext.len() - n..]
                .iter()
                .all(|&b| b == pad_len);
            if valid {
                plaintext.truncate(plaintext.len() - n);
            }
        }
    }

    plaintext
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_type_from_name() {
        assert!(BrowserType::from_name("chrome").is_some());
        assert!(BrowserType::from_name("Chrome").is_some());
        assert!(BrowserType::from_name("CHROME").is_some());
        assert!(BrowserType::from_name("arc").is_some());
        assert!(BrowserType::from_name("brave").is_some());
        assert!(BrowserType::from_name("edge").is_some());
        assert!(BrowserType::from_name("chromium").is_some());
        assert!(BrowserType::from_name("firefox").is_none());
        assert!(BrowserType::from_name("safari").is_none());
    }

    #[test]
    fn browser_type_display() {
        assert_eq!(BrowserType::Chrome.to_string(), "Chrome");
        assert_eq!(BrowserType::Arc.to_string(), "Arc");
    }

    #[test]
    fn decrypt_known_value() {
        // AES-128-CBC with key=0x00*16, IV=0x20*16 (Chromium uses space as IV)
        // Round-trip: encrypt with CBC using space IV, then decrypt should recover.
        use aes::Aes128;
        use aes::cipher::generic_array::GenericArray;
        use aes::cipher::{BlockEncrypt, KeyInit};

        let key = [0u8; 16];
        let iv = [0x20u8; 16]; // space IV, matching Chromium
        let cipher = Aes128::new(GenericArray::from_slice(&key));

        // "hello" + PKCS7 padding, XOR with IV for first block
        let mut padded = *b"hello\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b";
        for (i, byte) in padded.iter_mut().enumerate() {
            *byte ^= iv[i];
        }
        let mut block = GenericArray::clone_from_slice(&padded);
        cipher.encrypt_block(&mut block);
        let ciphertext: Vec<u8> = block.to_vec();

        // Decrypt should recover "hello"
        let plaintext = decrypt_aes_128_cbc(&ciphertext, &key);
        assert_eq!(plaintext, b"hello");

        // Full cookie format: "v10" prefix + ciphertext
        let mut encrypted = b"v10".to_vec();
        encrypted.extend_from_slice(&ciphertext);
        let result = decrypt_cookie_value(&encrypted, &key, 0).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn decrypt_unknown_prefix_falls_back_to_raw() {
        let key = [0u8; 16];
        let raw = b"plaintext_value";
        let result = decrypt_cookie_value(raw, &key, 0).unwrap();
        assert_eq!(result, "plaintext_value");
    }
}
