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
        // ESP-IDF mDNS implementation:
        //
        // use esp_idf_svc::mdns::EspMdns;
        //
        // let mut mdns = EspMdns::take().unwrap();
        // mdns.set_hostname(self.hostname.as_str()).unwrap();
        // mdns.set_instance_name("PetFilter Device").unwrap();
        // mdns.add_service(
        //     Some("PetFilter"),
        //     MDNS_SERVICE_TYPE,
        //     MDNS_SERVICE_PROTO,
        //     MDNS_SERVICE_PORT,
        //     &[
        //         ("version", env!("CARGO_PKG_VERSION")),
        //         ("device_id", self.device_id.as_str()),
        //     ],
        // ).unwrap();
        info!("mDNS(espidf): service registration deferred until WiFi wiring");
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_start(&self) {
        info!(
            "mDNS(sim): registered {}.local {}:{} v={} id={}",
            self.hostname, MDNS_SERVICE_TYPE, MDNS_SERVICE_PORT,
            env!("CARGO_PKG_VERSION"), self.device_id
        );
    }

    #[cfg(target_os = "espidf")]
    fn platform_stop(&self) {
        // mdns instance is dropped or service removed
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
