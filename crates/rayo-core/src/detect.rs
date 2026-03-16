//! Auto-detect the user's default web browser.
//!
//! Used by the transparent auth system to import cookies from the browser
//! the user actually logs in with.

#[cfg(feature = "cookie-import")]
use crate::cookie_import::BrowserType;

/// Detect the user's default web browser on macOS.
///
/// Uses `defaults read` to query the Launch Services handler for `https`.
/// Maps known bundle identifiers to [`BrowserType`].
#[cfg(all(target_os = "macos", feature = "cookie-import"))]
pub fn default_browser() -> Option<BrowserType> {
    // Use `defaults read` on the LSHandlers plist to find the https handler.
    let output = std::process::Command::new("defaults")
        .args([
            "read",
            "com.apple.LaunchServices/com.apple.launchservices.secure",
            "LSHandlers",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return find_available_browser();
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Look for the handler entry that contains "https" scheme
    // The plist output has blocks like:
    //   { LSHandlerRoleAll = "com.google.chrome"; LSHandlerURLScheme = https; }
    // We search for the block containing "https" and extract the bundle ID.
    let mut in_https_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("LSHandlerURLScheme") && trimmed.contains("https") {
            in_https_block = true;
        }
        if in_https_block && trimmed.contains("LSHandlerRoleAll") {
            // Extract bundle ID between quotes
            if let Some(start) = trimmed.find('"')
                && let Some(end) = trimmed[start + 1..].find('"')
            {
                let bundle_id = &trimmed[start + 1..start + 1 + end];
                return bundle_id_to_browser(bundle_id);
            }
            // Try without quotes (some plist formats)
            if let Some(eq_pos) = trimmed.find('=') {
                let after_eq = trimmed[eq_pos + 1..].trim().trim_end_matches(';').trim();
                let after_eq = after_eq.trim_matches('"');
                return bundle_id_to_browser(after_eq);
            }
        }
        // Also check if the bundle ID comes before the scheme in the same block
        if trimmed.contains("LSHandlerRoleAll") {
            in_https_block = true;
        }
        if in_https_block && trimmed.contains("LSHandlerURLScheme") && trimmed.contains("https") {
            // The bundle ID was in a previous line in this block — re-scan upward
            // This is hard with a forward iterator, so fall through to find_available_browser
        }
        // Block separator
        if trimmed == "}" || trimmed == "}," {
            in_https_block = false;
        }
    }

    // Fallback: try each browser in popularity order
    find_available_browser()
}

/// Map macOS bundle identifiers to BrowserType.
#[cfg(all(target_os = "macos", feature = "cookie-import"))]
fn bundle_id_to_browser(bundle_id: &str) -> Option<BrowserType> {
    let lower = bundle_id.to_lowercase();
    if lower.contains("com.google.chrome") {
        Some(BrowserType::Chrome)
    } else if lower.contains("company.thebrowser.browser") || lower.contains("arc") {
        Some(BrowserType::Arc)
    } else if lower.contains("com.brave") {
        Some(BrowserType::Brave)
    } else if lower.contains("com.microsoft.edgemac") || lower.contains("microsoft.edge") {
        Some(BrowserType::Edge)
    } else if lower.contains("org.chromium.chromium") || lower.contains("chromium") {
        Some(BrowserType::Chromium)
    } else {
        // Unknown browser (Safari, Firefox, etc.) — fall back to finding any available
        find_available_browser()
    }
}

/// Detect the user's default web browser on Linux.
///
/// Uses `xdg-settings get default-web-browser` and maps .desktop file names.
#[cfg(all(target_os = "linux", feature = "cookie-import"))]
pub fn default_browser() -> Option<BrowserType> {
    let output = std::process::Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .ok()?;

    if !output.status.success() {
        return find_available_browser();
    }

    let desktop = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase();

    if desktop.contains("google-chrome") {
        Some(BrowserType::Chrome)
    } else if desktop.contains("arc") {
        Some(BrowserType::Arc)
    } else if desktop.contains("brave") {
        Some(BrowserType::Brave)
    } else if desktop.contains("microsoft-edge") {
        Some(BrowserType::Edge)
    } else if desktop.contains("chromium") {
        Some(BrowserType::Chromium)
    } else {
        find_available_browser()
    }
}

/// Fallback for unsupported platforms.
#[cfg(all(
    not(target_os = "macos"),
    not(target_os = "linux"),
    feature = "cookie-import"
))]
pub fn default_browser() -> Option<BrowserType> {
    None
}

/// Try all known browsers and return the first one with an accessible cookie database.
///
/// Ordered by market share: Chrome, Arc, Brave, Edge, Chromium.
#[cfg(feature = "cookie-import")]
pub fn find_available_browser() -> Option<BrowserType> {
    let browsers = [
        BrowserType::Chrome,
        BrowserType::Arc,
        BrowserType::Brave,
        BrowserType::Edge,
        BrowserType::Chromium,
    ];

    for browser in &browsers {
        if !browser.list_profiles().is_empty() {
            return Some(*browser);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "cookie-import")]
    fn find_available_browser_does_not_panic() {
        // Should not panic regardless of what browsers are installed
        let _ = super::find_available_browser();
    }

    #[test]
    #[cfg(feature = "cookie-import")]
    fn default_browser_does_not_panic() {
        // Should not panic regardless of platform or config
        let _ = super::default_browser();
    }
}
