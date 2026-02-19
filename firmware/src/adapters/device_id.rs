//! Device identity derived from the ESP32 factory MAC address.
//!
//! Produces a stable, human-readable device ID in the form `PF-XXYYZZ`
//! (last 3 bytes of the 6-byte MAC in uppercase hex). This ID is:
//! - Deterministic across reboots (factory-burned eFuse MAC)
//! - Used as BLE advertising local name (`petfilter-XXYYZZ`)
//! - Used as mDNS hostname (`petfilter-xxyyzz.local`)
//! - Reported in `GetDeviceInfoResponse.serial_number`

/// Fixed-size device ID string: "PF-XXYYZZ" (9 chars + null = 10 max).
pub type DeviceIdString = heapless::String<16>;

/// Full 6-byte MAC address.
pub type MacAddress = [u8; 6];

/// Read the factory MAC address from eFuse.
#[cfg(target_os = "espidf")]
pub fn read_mac() -> MacAddress {
    let mut mac: MacAddress = [0u8; 6];
    unsafe {
        esp_idf_svc::sys::esp_efuse_mac_get_default(mac.as_mut_ptr());
    }
    mac
}

/// Simulation: returns a deterministic fake MAC.
#[cfg(not(target_os = "espidf"))]
pub fn read_mac() -> MacAddress {
    [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]
}

/// Derive the short device ID from the last 3 MAC bytes.
/// Format: `PF-XXYYZZ` (e.g., `PF-EFCAFE`).
pub fn device_id(mac: &MacAddress) -> DeviceIdString {
    let mut id = DeviceIdString::new();
    use core::fmt::Write;
    let _ = write!(id, "PF-{:02X}{:02X}{:02X}", mac[3], mac[4], mac[5]);
    id
}

/// Derive the BLE/mDNS hostname from the last 3 MAC bytes.
/// Format: `petfilter-xxyyzz` (lowercase, 16 chars max).
pub fn hostname(mac: &MacAddress) -> heapless::String<24> {
    let mut name = heapless::String::<24>::new();
    use core::fmt::Write;
    let _ = write!(name, "petfilter-{:02x}{:02x}{:02x}", mac[3], mac[4], mac[5]);
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_format() {
        let mac = [0x00, 0x11, 0x22, 0xAA, 0xBB, 0xCC];
        assert_eq!(device_id(&mac).as_str(), "PF-AABBCC");
    }

    #[test]
    fn hostname_format() {
        let mac = [0x00, 0x11, 0x22, 0xAA, 0xBB, 0xCC];
        assert_eq!(hostname(&mac).as_str(), "petfilter-aabbcc");
    }

    #[test]
    fn sim_mac_deterministic() {
        let m1 = read_mac();
        let m2 = read_mac();
        assert_eq!(m1, m2);
    }

    #[test]
    fn device_id_from_sim_mac() {
        let mac = read_mac();
        let id = device_id(&mac);
        assert_eq!(id.as_str(), "PF-EFCAFE");
    }
}
