//! TLS 1.3 transport adapter.
//!
//! Implements [`Transport`](crate::rpc::transport::Transport) — a
//! byte-oriented channel that listens for a single TCP client on a
//! configurable port and wraps the connection with TLS 1.3 using a
//! pre-shared key (PSK) cipher suite.
//!
//! ## cfg gating
//!
//! - **`target_os = "espidf"`**: real TCP listener + ESP-IDF mbedtls
//!   TLS 1.3 with PSK.  Single-client limit — only one RPC client
//!   at a time.
//! - **all other targets**: simulation stubs using `std::net::TcpListener`
//!   and `TcpStream` in plaintext (no TLS) for host-side testing.
//!
//! ## Connection model
//!
//! The transport operates as a non-blocking TCP server:
//!
//! 1. `new()` binds a TCP listener on the given port (non-blocking mode).
//! 2. `accept()` polls for an incoming connection; on success the client
//!    socket is wrapped in a TLS session and the state transitions to
//!    `Connected`.
//! 3. Reads are non-blocking — `read()` returns `Ok(0)` when no data
//!    is available rather than blocking the caller.
//! 4. `disconnect()` tears down the TLS session / TCP connection and
//!    returns the transport to `Listening`.

use core::fmt;
use log::{info, warn};

use crate::rpc::transport::Transport;

#[cfg(not(target_os = "espidf"))]
use std::io::{Read, Write};

// ───────────────────────────────────────────────────────────────
// Constants
// ───────────────────────────────────────────────────────────────

/// Maximum pre-shared key length (bytes).
const MAX_PSK_LEN: usize = 64;

/// Default listen port for the RPC TLS server.
pub const DEFAULT_PORT: u16 = 4242;

// ───────────────────────────────────────────────────────────────
// Error type
// ───────────────────────────────────────────────────────────────

/// Errors originating from the TLS transport layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsTransportError {
    /// TCP or socket I/O failure.
    Io,
    /// TLS handshake or session error.
    Tls,
    /// Operation requires a connected client but none is present.
    NotConnected,
    /// A client is already connected (single-client limit).
    AlreadyConnected,
}

impl fmt::Display for TlsTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io => write!(f, "TCP/socket I/O error"),
            Self::Tls => write!(f, "TLS handshake or session error"),
            Self::NotConnected => write!(f, "no client connected"),
            Self::AlreadyConnected => write!(f, "a client is already connected"),
        }
    }
}

// ───────────────────────────────────────────────────────────────
// Connection state
// ───────────────────────────────────────────────────────────────

/// TLS connection lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsConnectionState {
    /// Bound and listening for incoming connections.
    Listening,
    /// A client is connected and the TLS session is active.
    Connected,
    /// An unrecoverable error occurred on the transport.
    Error,
}

// ───────────────────────────────────────────────────────────────
// ESP-IDF platform helpers (real lwIP + mbedTLS)
// ───────────────────────────────────────────────────────────────
#[cfg(target_os = "espidf")]
mod esp_impl;

// ───────────────────────────────────────────────────────────────
// TlsTransport
// ───────────────────────────────────────────────────────────────

/// TLS 1.3 transport wrapping a single-client TCP server.
///
/// On ESP-IDF the TCP socket is wrapped with mbedtls for TLS 1.3
/// using a PSK cipher suite.  On host targets the connection is
/// plaintext for ease of testing.
pub struct TlsTransport {
    state: TlsConnectionState,
    psk: heapless::Vec<u8, MAX_PSK_LEN>,
    port: u16,

    // ── ESP-IDF fields ──────────────────────────────────────────
    #[cfg(target_os = "espidf")]
    server: Option<esp_impl::EspTlsServer>,
    #[cfg(target_os = "espidf")]
    client: Option<esp_impl::EspTlsClient>,

    // ── Simulation fields ───────────────────────────────────────
    #[cfg(not(target_os = "espidf"))]
    listener: std::net::TcpListener,
    #[cfg(not(target_os = "espidf"))]
    stream: Option<std::net::TcpStream>,
}

impl TlsTransport {
    // ── Constructors (platform-specific) ──────────────────────

    /// Create a new TLS transport bound to `0.0.0.0:<port>`.
    ///
    /// `psk` is the pre-shared key for the TLS 1.3 PSK cipher suite
    /// (max 64 bytes).  The TCP listener is set to non-blocking mode.
    #[cfg(target_os = "espidf")]
    pub fn new(port: u16, psk: &[u8]) -> Result<Self, TlsTransportError> {
        let mut psk_buf = heapless::Vec::new();
        psk_buf
            .extend_from_slice(psk)
            .map_err(|_| TlsTransportError::Tls)?;

        let server = esp_impl::esp_new(port, psk)?;

        Ok(Self {
            state: TlsConnectionState::Listening,
            psk: psk_buf,
            port,
            server: Some(server),
            client: None,
        })
    }

    /// Create a new TLS transport bound to `0.0.0.0:<port>`.
    ///
    /// On host targets TLS is not applied — the connection is plaintext
    /// for ease of testing.  Pass port `0` to let the OS pick a free
    /// port (use [`local_addr()`](Self::local_addr) to discover it).
    #[cfg(not(target_os = "espidf"))]
    pub fn new(port: u16, psk: &[u8]) -> Result<Self, TlsTransportError> {
        let mut psk_buf = heapless::Vec::new();
        psk_buf
            .extend_from_slice(psk)
            .map_err(|_| TlsTransportError::Tls)?;

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let listener =
            std::net::TcpListener::bind(addr).map_err(|_| TlsTransportError::Io)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| TlsTransportError::Io)?;

        info!("TLS(sim): listening on port {} (plaintext)", port);

        Ok(Self {
            state: TlsConnectionState::Listening,
            psk: psk_buf,
            port,
            listener,
            stream: None,
        })
    }

    // ── Public API ────────────────────────────────────────────

    /// Current connection state.
    pub fn state(&self) -> TlsConnectionState {
        self.state
    }

    /// Whether a client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.state == TlsConnectionState::Connected
    }

    /// Poll for an incoming client connection (non-blocking).
    ///
    /// Returns `true` if a new client was accepted and the TLS
    /// handshake succeeded.  Returns `false` if no client is
    /// waiting or a client is already connected.
    pub fn accept(&mut self) -> bool {
        if self.state == TlsConnectionState::Connected {
            return false;
        }
        self.platform_accept()
    }

    /// Disconnect the current client and return to `Listening`.
    pub fn disconnect(&mut self) {
        self.platform_disconnect();
        self.state = TlsConnectionState::Listening;
    }

    /// The actual bound address (simulation only).
    ///
    /// Useful when port `0` was passed to `new()`.
    #[cfg(not(target_os = "espidf"))]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.listener.local_addr().expect("listener has local address")
    }

    // ── Platform helpers: accept ──────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_accept(&mut self) -> bool {
        let server = match self.server.as_ref() {
            Some(s) => s,
            None => return false,
        };
        match esp_impl::esp_accept(server) {
            Some(client) => {
                self.client = Some(client);
                self.state = TlsConnectionState::Connected;
                true
            }
            None => false,
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_accept(&mut self) -> bool {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                if stream.set_nonblocking(true).is_err() {
                    warn!("TLS(sim): failed to set non-blocking on client socket");
                    return false;
                }
                info!("TLS(sim): client connected from {}", addr);
                self.stream = Some(stream);
                self.state = TlsConnectionState::Connected;
                true
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(e) => {
                warn!("TLS(sim): accept error: {}", e);
                false
            }
        }
    }

    // ── Platform helpers: disconnect ──────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_disconnect(&mut self) {
        // EspTlsClient::drop() calls ssl_close_notify + ssl_free + lwip_close.
        if self.client.take().is_some() {
            info!("TLS(espidf): client disconnected");
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_disconnect(&mut self) {
        if self.stream.take().is_some() {
            info!("TLS(sim): client disconnected");
        }
    }

    // ── Platform helpers: read ────────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_read(&mut self, buf: &mut [u8]) -> Result<usize, TlsTransportError> {
        let client = self.client.as_mut().ok_or(TlsTransportError::NotConnected)?;
        match esp_impl::esp_read(client, buf) {
            Ok(n) => Ok(n),
            Err(TlsTransportError::NotConnected) => {
                // Peer closed — tear down and return to Listening state.
                self.client.take();
                self.state = TlsConnectionState::Listening;
                Err(TlsTransportError::NotConnected)
            }
            Err(e) => {
                self.client.take();
                self.state = TlsConnectionState::Error;
                Err(e)
            }
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_read(&mut self, buf: &mut [u8]) -> Result<usize, TlsTransportError> {
        let stream = self.stream.as_mut().ok_or(TlsTransportError::NotConnected)?;
        match stream.read(buf) {
            Ok(0) => {
                info!("TLS(sim): client disconnected (EOF)");
                self.stream.take();
                self.state = TlsConnectionState::Listening;
                Err(TlsTransportError::NotConnected)
            }
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(_) => {
                self.stream.take();
                self.state = TlsConnectionState::Error;
                Err(TlsTransportError::Io)
            }
        }
    }

    // ── Platform helpers: write ───────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_write(&mut self, data: &[u8]) -> Result<usize, TlsTransportError> {
        let client = self.client.as_mut().ok_or(TlsTransportError::NotConnected)?;
        match esp_impl::esp_write(client, data) {
            Ok(n) => Ok(n),
            Err(e) => {
                self.client.take();
                self.state = TlsConnectionState::Error;
                Err(e)
            }
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_write(&mut self, data: &[u8]) -> Result<usize, TlsTransportError> {
        let stream = self.stream.as_mut().ok_or(TlsTransportError::NotConnected)?;
        match stream.write(data) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(_) => {
                self.stream.take();
                self.state = TlsConnectionState::Error;
                Err(TlsTransportError::Io)
            }
        }
    }

    // ── Platform helpers: flush ───────────────────────────────

    #[cfg(target_os = "espidf")]
    fn platform_flush(&mut self) -> Result<(), TlsTransportError> {
        // mbedtls manages its own output buffering; no application-layer flush needed.
        Ok(())
    }

    #[cfg(not(target_os = "espidf"))]
    fn platform_flush(&mut self) -> Result<(), TlsTransportError> {
        let stream = self.stream.as_mut().ok_or(TlsTransportError::NotConnected)?;
        stream.flush().map_err(|_| TlsTransportError::Io)
    }
}

// ───────────────────────────────────────────────────────────────
// Transport implementation
// ───────────────────────────────────────────────────────────────

impl Transport for TlsTransport {
    type Error = TlsTransportError;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TlsTransportError> {
        if self.state != TlsConnectionState::Connected {
            return Err(TlsTransportError::NotConnected);
        }
        self.platform_read(buf)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, TlsTransportError> {
        if self.state != TlsConnectionState::Connected {
            return Err(TlsTransportError::NotConnected);
        }
        self.platform_write(data)
    }

    fn flush(&mut self) -> Result<(), TlsTransportError> {
        if self.state != TlsConnectionState::Connected {
            return Err(TlsTransportError::NotConnected);
        }
        self.platform_flush()
    }

    fn available(&self) -> bool {
        self.state == TlsConnectionState::Connected
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
    fn new_starts_in_listening_state() {
        let t = make_transport();
        assert_eq!(t.state(), TlsConnectionState::Listening);
        assert!(!t.is_connected());
    }

    #[test]
    fn accept_when_no_client_returns_false() {
        let mut t = make_transport();
        assert!(!t.accept());
        assert_eq!(t.state(), TlsConnectionState::Listening);
    }

    #[test]
    fn accept_and_disconnect_roundtrip() {
        let mut t = make_transport();
        let addr = t.local_addr();

        let _client = std::net::TcpStream::connect(addr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        assert!(t.accept());
        assert!(t.is_connected());
        assert!(t.available());
        assert_eq!(t.state(), TlsConnectionState::Connected);

        assert!(!t.accept());

        t.disconnect();
        assert_eq!(t.state(), TlsConnectionState::Listening);
        assert!(!t.is_connected());
    }

    #[test]
    fn read_and_write_while_connected() {
        let mut t = make_transport();
        let addr = t.local_addr();

        let mut client = std::net::TcpStream::connect(addr).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(t.accept());

        use std::io::Write as _;
        client.write_all(b"ping").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut buf = [0u8; 32];
        let n = t.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"ping");

        t.write(b"pong").unwrap();
        t.flush().unwrap();
        use std::io::Read as _;
        let mut reply = [0u8; 4];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"pong");
    }

    #[test]
    fn read_without_connection_returns_not_connected() {
        let mut t = make_transport();
        let mut buf = [0u8; 8];
        assert_eq!(t.read(&mut buf), Err(TlsTransportError::NotConnected));
    }

    #[test]
    fn write_without_connection_returns_not_connected() {
        let mut t = make_transport();
        assert_eq!(t.write(b"data"), Err(TlsTransportError::NotConnected));
    }
}
