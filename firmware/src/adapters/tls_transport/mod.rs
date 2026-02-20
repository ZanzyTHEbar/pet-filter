//! TLS 1.3 transport adapter — multi-client support.
//!
//! Implements a non-blocking TCP server with TLS 1.3 PSK encryption,
//! supporting up to [`MAX_CLIENTS`](crate::rpc::auth::MAX_CLIENTS)
//! concurrent connections.
//!
//! ## cfg gating
//!
//! - **`target_os = "espidf"`**: real TCP listener + ESP-IDF mbedtls
//!   TLS 1.3 with PSK.
//! - **all other targets**: simulation stubs using `std::net` in
//!   plaintext (no TLS) for host-side testing.
//!
//! ## Connection model
//!
//! 1. `new()` binds a TCP listener on the given port (non-blocking).
//! 2. `try_accept()` polls for incoming connections; on success the
//!    socket is TLS-wrapped and assigned a client ID.
//! 3. Reads/writes are non-blocking and addressed by client ID.
//! 4. `disconnect(client_id)` tears down a specific connection.

use core::fmt;
use log::{info, warn};

use crate::rpc::auth::{ClientId, MAX_CLIENTS};
use crate::rpc::transport::Transport;

#[cfg(not(target_os = "espidf"))]
use std::io::{Read, Write};

// ───────────────────────────────────────────────────────────────
// Constants
// ───────────────────────────────────────────────────────────────

const MAX_PSK_LEN: usize = 64;

pub const DEFAULT_PORT: u16 = 4242;

// ───────────────────────────────────────────────────────────────
// Error type
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsTransportError {
    Io,
    Tls,
    NotConnected,
    AlreadyConnected,
    NoSlotsAvailable,
}

impl fmt::Display for TlsTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io => write!(f, "TCP/socket I/O error"),
            Self::Tls => write!(f, "TLS handshake or session error"),
            Self::NotConnected => write!(f, "no client connected"),
            Self::AlreadyConnected => write!(f, "a client is already connected"),
            Self::NoSlotsAvailable => write!(f, "all client slots occupied"),
        }
    }
}

// ───────────────────────────────────────────────────────────────
// Connection state
// ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsConnectionState {
    Listening,
    Connected,
    Error,
}

// ───────────────────────────────────────────────────────────────
// ESP-IDF platform helpers
// ───────────────────────────────────────────────────────────────
#[cfg(target_os = "espidf")]
mod esp_impl;

// ───────────────────────────────────────────────────────────────
// Per-client slot
// ───────────────────────────────────────────────────────────────

/// Internal per-client connection state.
struct ClientSlot {
    state: TlsConnectionState,

    #[cfg(target_os = "espidf")]
    tls_client: Option<esp_impl::EspTlsClient>,

    #[cfg(not(target_os = "espidf"))]
    stream: Option<std::net::TcpStream>,
}

impl ClientSlot {
    fn new() -> Self {
        Self {
            state: TlsConnectionState::Listening,
            #[cfg(target_os = "espidf")]
            tls_client: None,
            #[cfg(not(target_os = "espidf"))]
            stream: None,
        }
    }

    fn is_free(&self) -> bool {
        self.state == TlsConnectionState::Listening
    }

    fn disconnect(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            self.tls_client.take();
        }
        #[cfg(not(target_os = "espidf"))]
        {
            self.stream.take();
        }
        self.state = TlsConnectionState::Listening;
    }
}

// ───────────────────────────────────────────────────────────────
// Multi-client TlsTransport
// ───────────────────────────────────────────────────────────────

/// Multi-client TLS 1.3 server transport.
#[allow(dead_code)]
pub struct TlsTransport {
    psk: heapless::Vec<u8, MAX_PSK_LEN>,
    port: u16,
    clients: [ClientSlot; MAX_CLIENTS],

    #[cfg(target_os = "espidf")]
    server: Option<esp_impl::EspTlsServer>,

    #[cfg(not(target_os = "espidf"))]
    listener: std::net::TcpListener,
}

impl TlsTransport {
    #[cfg(target_os = "espidf")]
    pub fn new(port: u16, psk: &[u8]) -> Result<Self, TlsTransportError> {
        let mut psk_buf = heapless::Vec::new();
        psk_buf
            .extend_from_slice(psk)
            .map_err(|_| TlsTransportError::Tls)?;

        let server = esp_impl::esp_new(port, psk)?;

        Ok(Self {
            psk: psk_buf,
            port,
            clients: core::array::from_fn(|_| ClientSlot::new()),
            server: Some(server),
        })
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn new(port: u16, psk: &[u8]) -> Result<Self, TlsTransportError> {
        let mut psk_buf = heapless::Vec::new();
        psk_buf
            .extend_from_slice(psk)
            .map_err(|_| TlsTransportError::Tls)?;

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let listener = std::net::TcpListener::bind(addr).map_err(|_| TlsTransportError::Io)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| TlsTransportError::Io)?;

        info!(
            "TLS(sim): listening on port {} (plaintext, max {} clients)",
            port, MAX_CLIENTS
        );

        Ok(Self {
            psk: psk_buf,
            port,
            clients: core::array::from_fn(|_| ClientSlot::new()),
            listener,
        })
    }

    // ── Public API ────────────────────────────────────────────

    /// Try to accept a new client. Returns the assigned `ClientId` on success.
    /// Slot 0 is reserved for BLE; TCP clients are assigned to slots 1..MAX_CLIENTS.
    pub fn try_accept(&mut self) -> Option<ClientId> {
        let slot_idx = self
            .clients
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, s)| s.is_free())
            .map(|(i, _)| i)?;
        if self.platform_accept(slot_idx) {
            Some(slot_idx as ClientId)
        } else {
            None
        }
    }

    /// Disconnect a specific client.
    pub fn disconnect(&mut self, client_id: ClientId) {
        if let Some(slot) = self.clients.get_mut(client_id as usize) {
            if slot.state == TlsConnectionState::Connected {
                info!("TLS: disconnecting client {}", client_id);
            }
            slot.disconnect();
        }
    }

    /// Read from a specific client (non-blocking).
    pub fn read_client(
        &mut self,
        client_id: ClientId,
        buf: &mut [u8],
    ) -> Result<usize, TlsTransportError> {
        let slot = self
            .clients
            .get_mut(client_id as usize)
            .ok_or(TlsTransportError::NotConnected)?;
        if slot.state != TlsConnectionState::Connected {
            return Err(TlsTransportError::NotConnected);
        }
        self.platform_read(client_id, buf)
    }

    /// Write to a specific client.
    pub fn write_client(
        &mut self,
        client_id: ClientId,
        data: &[u8],
    ) -> Result<usize, TlsTransportError> {
        let slot = self
            .clients
            .get_mut(client_id as usize)
            .ok_or(TlsTransportError::NotConnected)?;
        if slot.state != TlsConnectionState::Connected {
            return Err(TlsTransportError::NotConnected);
        }
        self.platform_write(client_id, data)
    }

    /// Flush a specific client's output buffer.
    pub fn flush_client(&mut self, client_id: ClientId) -> Result<(), TlsTransportError> {
        let slot = self
            .clients
            .get_mut(client_id as usize)
            .ok_or(TlsTransportError::NotConnected)?;
        if slot.state != TlsConnectionState::Connected {
            return Err(TlsTransportError::NotConnected);
        }
        self.platform_flush(client_id)
    }

    /// Whether a specific client is connected.
    pub fn is_connected(&self, client_id: ClientId) -> bool {
        self.clients
            .get(client_id as usize)
            .is_some_and(|s| s.state == TlsConnectionState::Connected)
    }

    /// Number of currently connected clients.
    pub fn connected_count(&self) -> usize {
        self.clients
            .iter()
            .filter(|s| s.state == TlsConnectionState::Connected)
            .count()
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.listener
            .local_addr()
            .expect("listener has local address")
    }

    // ── Platform helpers: accept ──────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_accept(&mut self, slot_idx: usize) -> bool {
        let server = match self.server.as_ref() {
            Some(s) => s,
            None => return false,
        };
        match esp_impl::esp_accept(server) {
            Some(client) => {
                self.clients[slot_idx].tls_client = Some(client);
                self.clients[slot_idx].state = TlsConnectionState::Connected;
                info!("TLS(espidf): client {} connected", slot_idx);
                true
            }
            None => false,
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_accept(&mut self, slot_idx: usize) -> bool {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                if stream.set_nonblocking(true).is_err() {
                    warn!("TLS(sim): failed to set non-blocking on client socket");
                    return false;
                }
                info!("TLS(sim): client {} connected from {}", slot_idx, addr);
                self.clients[slot_idx].stream = Some(stream);
                self.clients[slot_idx].state = TlsConnectionState::Connected;
                true
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(e) => {
                warn!("TLS(sim): accept error: {}", e);
                false
            }
        }
    }

    // ── Platform helpers: read ────────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_read(
        &mut self,
        client_id: ClientId,
        buf: &mut [u8],
    ) -> Result<usize, TlsTransportError> {
        let slot = &mut self.clients[client_id as usize];
        let client = slot
            .tls_client
            .as_mut()
            .ok_or(TlsTransportError::NotConnected)?;
        match esp_impl::esp_read(client, buf) {
            Ok(n) => Ok(n),
            Err(TlsTransportError::NotConnected) => {
                slot.disconnect();
                Err(TlsTransportError::NotConnected)
            }
            Err(e) => {
                slot.state = TlsConnectionState::Error;
                Err(e)
            }
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_read(
        &mut self,
        client_id: ClientId,
        buf: &mut [u8],
    ) -> Result<usize, TlsTransportError> {
        let slot = &mut self.clients[client_id as usize];
        let stream = slot
            .stream
            .as_mut()
            .ok_or(TlsTransportError::NotConnected)?;
        match stream.read(buf) {
            Ok(0) => {
                info!("TLS(sim): client {} disconnected (EOF)", client_id);
                slot.disconnect();
                Err(TlsTransportError::NotConnected)
            }
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(_) => {
                slot.state = TlsConnectionState::Error;
                Err(TlsTransportError::Io)
            }
        }
    }

    // ── Platform helpers: write ───────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_write(
        &mut self,
        client_id: ClientId,
        data: &[u8],
    ) -> Result<usize, TlsTransportError> {
        let slot = &mut self.clients[client_id as usize];
        let client = slot
            .tls_client
            .as_mut()
            .ok_or(TlsTransportError::NotConnected)?;
        match esp_impl::esp_write(client, data) {
            Ok(n) => Ok(n),
            Err(e) => {
                slot.state = TlsConnectionState::Error;
                Err(e)
            }
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_write(
        &mut self,
        client_id: ClientId,
        data: &[u8],
    ) -> Result<usize, TlsTransportError> {
        let slot = &mut self.clients[client_id as usize];
        let stream = slot
            .stream
            .as_mut()
            .ok_or(TlsTransportError::NotConnected)?;
        match stream.write(data) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(_) => {
                slot.state = TlsConnectionState::Error;
                Err(TlsTransportError::Io)
            }
        }
    }

    // ── Platform helpers: flush ───────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_flush(&mut self, _client_id: ClientId) -> Result<(), TlsTransportError> {
        Ok(())
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_flush(&mut self, client_id: ClientId) -> Result<(), TlsTransportError> {
        let slot = &mut self.clients[client_id as usize];
        let stream = slot
            .stream
            .as_mut()
            .ok_or(TlsTransportError::NotConnected)?;
        stream.flush().map_err(|_| TlsTransportError::Io)
    }
}

// ───────────────────────────────────────────────────────────────
// Legacy single-client Transport trait (backward compat)
// ───────────────────────────────────────────────────────────────

impl Transport for TlsTransport {
    type Error = TlsTransportError;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TlsTransportError> {
        for i in 0..MAX_CLIENTS {
            if self.is_connected(i as ClientId) {
                let n = self.read_client(i as ClientId, buf)?;
                if n > 0 {
                    return Ok(n);
                }
            }
        }
        Ok(0)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, TlsTransportError> {
        for i in 0..MAX_CLIENTS {
            if self.is_connected(i as ClientId) {
                return self.write_client(i as ClientId, data);
            }
        }
        Err(TlsTransportError::NotConnected)
    }

    fn flush(&mut self) -> Result<(), TlsTransportError> {
        for i in 0..MAX_CLIENTS {
            if self.is_connected(i as ClientId) {
                self.flush_client(i as ClientId)?;
            }
        }
        Ok(())
    }

    fn available(&self) -> bool {
        self.connected_count() > 0
    }
}

// ───────────────────────────────────────────────────────────────
// Tests (host / simulation path only)
// ───────────────────────────────────────────────────────────────

#[cfg(all(test, not(target_os = "espidf")))]
mod tests {
    use super::*;

    fn make_transport() -> TlsTransport {
        TlsTransport::new(0, b"test-psk-key").unwrap()
    }

    #[test]
    fn new_starts_with_no_clients() {
        let t = make_transport();
        assert_eq!(t.connected_count(), 0);
    }

    #[test]
    fn accept_when_no_client_returns_none() {
        let mut t = make_transport();
        assert!(t.try_accept().is_none());
    }

    #[test]
    fn multi_client_accept_and_disconnect() {
        let mut t = make_transport();
        let addr = t.local_addr();

        let _c1 = std::net::TcpStream::connect(addr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let id1 = t.try_accept().unwrap();
        assert_eq!(id1, 1); // slot 0 reserved for BLE
        assert_eq!(t.connected_count(), 1);

        let _c2 = std::net::TcpStream::connect(addr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let id2 = t.try_accept().unwrap();
        assert_eq!(id2, 2);
        assert_eq!(t.connected_count(), 2);

        t.disconnect(id1);
        assert_eq!(t.connected_count(), 1);
        assert!(!t.is_connected(id1));
        assert!(t.is_connected(id2));
    }

    #[test]
    fn read_write_specific_client() {
        let mut t = make_transport();
        let addr = t.local_addr();

        let mut client = std::net::TcpStream::connect(addr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let cid = t.try_accept().unwrap();

        use std::io::Write as _;
        client.write_all(b"hello").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut buf = [0u8; 32];
        let n = t.read_client(cid, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"hello");

        t.write_client(cid, b"world").unwrap();
        t.flush_client(cid).unwrap();

        use std::io::Read as _;
        let mut reply = [0u8; 5];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"world");
    }

    #[test]
    fn max_clients_rejection() {
        let mut t = make_transport();
        let addr = t.local_addr();

        // Slot 0 is reserved for BLE — only (MAX_CLIENTS - 1) TCP slots available
        let tcp_slots = MAX_CLIENTS - 1;
        let mut clients = Vec::new();
        for _ in 0..tcp_slots {
            let c = std::net::TcpStream::connect(addr).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            t.try_accept().unwrap();
            clients.push(c);
        }

        assert_eq!(t.connected_count(), tcp_slots);

        let _overflow = std::net::TcpStream::connect(addr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(t.try_accept().is_none());
    }
}
