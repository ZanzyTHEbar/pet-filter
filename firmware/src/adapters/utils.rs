//! Shared utilities for adapter-layer validation.
//!
//! These helpers are used by multiple adapters (WiFi, BLE) and are
//! centralised here to avoid duplication.

/// Returns `true` if every byte of `s` is in the printable ASCII range
/// `0x20..=0x7E` (space through tilde, inclusive).
///
/// Used to validate WiFi SSID strings and BLE-provisioned credentials.
pub(super) fn is_printable_ascii(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_ascii_accepts_normal_strings() {
        assert!(is_printable_ascii("HomeNetwork"));
        assert!(is_printable_ascii("MySSID-2.4GHz"));
        assert!(is_printable_ascii("P@$$w0rd!"));
    }

    #[test]
    fn printable_ascii_rejects_control_chars() {
        assert!(!is_printable_ascii("bad\x00ssid"));
        assert!(!is_printable_ascii("tab\there"));
        assert!(!is_printable_ascii("newline\nhere"));
    }

    #[test]
    fn printable_ascii_rejects_high_bytes() {
        assert!(!is_printable_ascii("caf\u{e9}"));
    }

    #[test]
    fn empty_string_is_printable_ascii() {
        assert!(is_printable_ascii(""));
    }
}
