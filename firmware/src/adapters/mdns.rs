//! mDNS service advertisement adapter.
//!
//! Advertises `_petfilter._tcp` on port 4242 with TXT records for
//! device version and ID. Uses `esp-idf-svc` mDNS wrapper on ESP-IDF
//! and is a no-op on simulation targets.
//!
//! Lifecycle is tied to WiFi: start on connect, stop on disconnect
//! or sleep entry.

use log::info;

const MDNS_SERVICE_TYPE: &str = "_petfilter";
#[allow(dead_code)]
const MDNS_SERVICE_PROTO: &str = "_tcp";
const MDNS_SERVICE_PORT: u16 = 4242;

/// mDNS advertisement adapter.
pub struct MdnsAdapter {
    hostname: heapless::String<24>,
    device_id: heapless::String<16>,
    active: bool,
}

impl MdnsAdapter {
    pub fn new(hostname: heapless::String<24>, device_id: heapless::String<16>) -> Self {
        Self {
            hostname,
            device_id,
            active: false,
        }
    }

    /// Whether mDNS is currently advertising.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Start mDNS hostname + service advertisement.
    /// Call after WiFi is connected and has an IP.
    pub fn start(&mut self) {
        if self.active {
            return;
        }
        self.platform_start();
        self.active = true;
        info!(
            "mDNS: advertising {}.local → {}:{} (device={})",
            self.hostname, MDNS_SERVICE_TYPE, MDNS_SERVICE_PORT, self.device_id
        );
    }

    /// Stop mDNS advertisement.
    /// Call before WiFi disconnect or sleep entry.
    pub fn stop(&mut self) {
        if !self.active {
            return;
        }
        self.platform_stop();
        self.active = false;
        info!("mDNS: stopped");
    }

    // ── Platform-specific ─────────────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_start(&self) {
        use esp_idf_svc::sys::*;
        unsafe {
            let ret = mdns_init();
            if ret != ESP_OK as i32 {
                log::error!("mDNS: mdns_init failed ({})", ret);
                return;
            }

            let mut hostname_buf = [0u8; 32];
            let hb = self.hostname.as_bytes();
            let hl = hb.len().min(31);
            hostname_buf[..hl].copy_from_slice(&hb[..hl]);
            mdns_hostname_set(hostname_buf.as_ptr() as *const _);
            mdns_instance_name_set(b"PetFilter Device\0".as_ptr() as *const _);

            let svc_type = b"_petfilter\0";
            let svc_proto = b"_tcp\0";
            mdns_service_add(
                b"PetFilter\0".as_ptr() as *const _,
                svc_type.as_ptr() as *const _,
                svc_proto.as_ptr() as *const _,
                MDNS_SERVICE_PORT,
                core::ptr::null_mut(),
                0,
            );

            // Add TXT records.
            let ver = concat!(env!("CARGO_PKG_VERSION"), "\0");
            let mut id_buf = [0u8; 24];
            let ib = self.device_id.as_bytes();
            let il = ib.len().min(23);
            id_buf[..il].copy_from_slice(&ib[..il]);

            mdns_service_txt_item_set(
                svc_type.as_ptr() as *const _,
                svc_proto.as_ptr() as *const _,
                b"version\0".as_ptr() as *const _,
                ver.as_ptr() as *const _,
            );
            mdns_service_txt_item_set(
                svc_type.as_ptr() as *const _,
                svc_proto.as_ptr() as *const _,
                b"id\0".as_ptr() as *const _,
                id_buf.as_ptr() as *const _,
            );
        }
        info!(
            "mDNS(espidf): registered {}.local {}:{} v={}",
            self.hostname,
            MDNS_SERVICE_TYPE,
            MDNS_SERVICE_PORT,
            env!("CARGO_PKG_VERSION")
        );
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_start(&self) {
        info!(
            "mDNS(sim): registered {}.local {}:{} v={} id={}",
            self.hostname,
            MDNS_SERVICE_TYPE,
            MDNS_SERVICE_PORT,
            env!("CARGO_PKG_VERSION"),
            self.device_id
        );
    }

    #[cfg(target_os = "espidf")]
    fn platform_stop(&self) {
        unsafe {
            esp_idf_svc::sys::mdns_free();
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_stop(&self) {
        info!("mDNS(sim): unregistered");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adapter() -> MdnsAdapter {
        let mut hostname = heapless::String::<24>::new();
        hostname.push_str("petfilter-aabbcc").ok();
        let mut device_id = heapless::String::<16>::new();
        device_id.push_str("PF-AABBCC").ok();
        MdnsAdapter::new(hostname, device_id)
    }

    #[test]
    fn start_stop_lifecycle() {
        let mut m = make_adapter();
        assert!(!m.is_active());
        m.start();
        assert!(m.is_active());
        m.stop();
        assert!(!m.is_active());
    }

    #[test]
    fn double_start_is_idempotent() {
        let mut m = make_adapter();
        m.start();
        m.start(); // no panic, still active
        assert!(m.is_active());
    }

    #[test]
    fn double_stop_is_idempotent() {
        let mut m = make_adapter();
        m.stop(); // not active, no panic
        assert!(!m.is_active());
    }
}
