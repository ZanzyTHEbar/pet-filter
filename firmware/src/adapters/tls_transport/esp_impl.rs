//! ESP-IDF platform helpers for `TlsTransport`.
//!
//! This module is compiled only for `target_os = "espidf"` and contains the
//! real lwIP TCP socket + mbedTLS TLS 1.3 PSK implementation.
//!
//! All public items are `pub(super)` to keep them private to the adapters
//! module.

use log::{info, warn};

use esp_idf_svc::sys::{
    AF_INET, EAGAIN, F_SETFL, MBEDTLS_ERR_SSL_PEER_CLOSE_NOTIFY, MBEDTLS_ERR_SSL_WANT_READ,
    MBEDTLS_ERR_SSL_WANT_WRITE, MBEDTLS_SSL_IS_SERVER, MBEDTLS_SSL_PRESET_DEFAULT,
    MBEDTLS_SSL_TLS1_3_KEY_EXCHANGE_MODE_PSK, MBEDTLS_SSL_TRANSPORT_STREAM,
    MBEDTLS_SSL_VERIFY_NONE, O_NONBLOCK, SOCK_STREAM, in_addr, lwip_accept, lwip_bind, lwip_close,
    lwip_fcntl, lwip_htons, lwip_listen, lwip_recv, lwip_send, lwip_socket,
    mbedtls_ctr_drbg_context, mbedtls_ctr_drbg_free, mbedtls_ctr_drbg_init,
    mbedtls_ctr_drbg_random, mbedtls_ctr_drbg_seed, mbedtls_entropy_context, mbedtls_entropy_free,
    mbedtls_entropy_func, mbedtls_entropy_init, mbedtls_ssl_close_notify,
    mbedtls_ssl_conf_authmode, mbedtls_ssl_conf_psk, mbedtls_ssl_conf_rng,
    mbedtls_ssl_conf_tls13_key_exchange_modes, mbedtls_ssl_config, mbedtls_ssl_config_defaults,
    mbedtls_ssl_config_free, mbedtls_ssl_config_init, mbedtls_ssl_context, mbedtls_ssl_free,
    mbedtls_ssl_handshake, mbedtls_ssl_init, mbedtls_ssl_read, mbedtls_ssl_set_bio,
    mbedtls_ssl_setup, mbedtls_ssl_write, sockaddr_in, vTaskDelay,
};

use super::TlsTransportError;

/// Maximum TLS handshake retries before giving up (non-blocking loop).
const HANDSHAKE_MAX_RETRIES: u32 = 100;

/// PSK identity string presented during TLS 1.3 PSK negotiation.
const PSK_IDENTITY: &[u8] = b"petfilter-rpc";

// ── BIO callbacks ─────────────────────────────────────────────────────────────
//
// mbedTLS calls these to send/receive raw bytes over the underlying transport.
// We pass the socket fd as a void pointer via the `p_bio` context.

/// mbedTLS send BIO callback — writes `len` bytes to the client socket.
///
/// # Safety
///
/// `ctx` must be a raw file descriptor cast to `*mut c_void`.  The fd must
/// remain valid for the lifetime of the TLS session (guaranteed by
/// `EspTlsClient::fd`).  No aliasing occurs because mbedTLS serialises all
/// BIO calls within a single `mbedtls_ssl_handshake` / `mbedtls_ssl_write`
/// invocation.
unsafe extern "C" fn bio_send(
    ctx: *mut core::ffi::c_void,
    buf: *const u8,
    len: usize,
) -> core::ffi::c_int {
    let fd = ctx as core::ffi::c_int;
    unsafe { lwip_send(fd, buf as *const core::ffi::c_void, len, 0) as core::ffi::c_int }
}

/// mbedTLS recv BIO callback — reads up to `len` bytes from the client socket.
///
/// # Safety
///
/// Same invariants as `bio_send`.  EAGAIN is mapped to
/// `MBEDTLS_ERR_SSL_WANT_READ` so the non-blocking handshake loop works.
unsafe extern "C" fn bio_recv(
    ctx: *mut core::ffi::c_void,
    buf: *mut u8,
    len: usize,
) -> core::ffi::c_int {
    let fd = ctx as core::ffi::c_int;
    let ret = unsafe { lwip_recv(fd, buf as *mut core::ffi::c_void, len, 0) } as core::ffi::c_int;
    if ret < 0 {
        // SAFETY: __errno() returns the pointer to the current task errno,
        // which is valid to read in any task context.
        let err = unsafe { *esp_idf_svc::sys::__errno() };
        if err == EAGAIN as i32 {
            return MBEDTLS_ERR_SSL_WANT_READ;
        }
    }
    ret
}

// ── Session state ─────────────────────────────────────────────────────────────

/// Per-connection TLS state (heap-allocated to limit stack pressure).
pub(super) struct EspTlsClient {
    pub(super) fd: core::ffi::c_int,
    pub(super) ssl: Box<mbedtls_ssl_context>,
}

unsafe impl Send for EspTlsClient {}

impl Drop for EspTlsClient {
    fn drop(&mut self) {
        // SAFETY: `ssl` was initialised by `mbedtls_ssl_init`/`mbedtls_ssl_setup`
        // and `fd` is a valid socket — both are freed/closed exactly once here.
        unsafe {
            mbedtls_ssl_close_notify(self.ssl.as_mut());
            mbedtls_ssl_free(self.ssl.as_mut());
            lwip_close(self.fd);
        }
    }
}

// ── Server (listener + TLS config) ───────────────────────────────────────────

/// Long-lived TLS server context (listener socket + mbedTLS config + RNG).
///
/// Owned by `TlsTransport`; dropped when the transport is destroyed.
pub(super) struct EspTlsServer {
    pub(super) listener_fd: core::ffi::c_int,
    pub(super) conf: Box<mbedtls_ssl_config>,
    entropy: Box<mbedtls_entropy_context>,
    drbg: Box<mbedtls_ctr_drbg_context>,
}

unsafe impl Send for EspTlsServer {}

impl Drop for EspTlsServer {
    fn drop(&mut self) {
        // SAFETY: All fields were initialised in `esp_new` and freed exactly once.
        unsafe {
            mbedtls_ssl_config_free(self.conf.as_mut());
            mbedtls_entropy_free(self.entropy.as_mut());
            mbedtls_ctr_drbg_free(self.drbg.as_mut());
            lwip_close(self.listener_fd);
        }
    }
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Bind a TCP listener and initialise the mbedTLS config with PSK.
///
/// Returns `Ok(EspTlsServer)` on success.
pub(super) fn esp_new(port: u16, psk: &[u8]) -> Result<EspTlsServer, TlsTransportError> {
    // ── Step 1: bind TCP listener ─────────────────────────────────────────
    // SAFETY: lwIP socket call with valid domain/type/protocol.
    let listener_fd = unsafe { lwip_socket(AF_INET as _, SOCK_STREAM as _, 0) };
    if listener_fd < 0 {
        warn!("TLS(espidf): lwip_socket failed ({})", listener_fd);
        return Err(TlsTransportError::Io);
    }

    let addr = sockaddr_in {
        sin_len: 0,
        sin_family: AF_INET as _,
        // SAFETY: lwip_htons takes a u16 and returns the network-byte-order value.
        sin_port: unsafe { lwip_htons(port) },
        sin_addr: in_addr { s_addr: 0 }, // INADDR_ANY = 0
        sin_zero: [0u8; 8],
    };

    // SAFETY: `addr` is valid for the bind() call; errors are checked.
    let rc = unsafe {
        lwip_bind(
            listener_fd,
            core::ptr::addr_of!(addr).cast(),
            core::mem::size_of::<sockaddr_in>() as _,
        )
    };
    if rc < 0 {
        unsafe {
            lwip_close(listener_fd);
        }
        warn!("TLS(espidf): lwip_bind failed ({})", rc);
        return Err(TlsTransportError::Io);
    }

    // SAFETY: listen() with backlog=1 for a single-client server.
    let rc = unsafe { lwip_listen(listener_fd, 1) };
    if rc < 0 {
        unsafe {
            lwip_close(listener_fd);
        }
        warn!("TLS(espidf): lwip_listen failed ({})", rc);
        return Err(TlsTransportError::Io);
    }

    // Make the listener non-blocking so accept() returns immediately.
    // SAFETY: F_SETFL + O_NONBLOCK is a valid fcntl for sockets.
    let rc = unsafe { lwip_fcntl(listener_fd, F_SETFL as _, O_NONBLOCK as _) };
    if rc < 0 {
        unsafe {
            lwip_close(listener_fd);
        }
        warn!("TLS(espidf): O_NONBLOCK failed ({})", rc);
        return Err(TlsTransportError::Io);
    }

    // ── Step 2: mbedTLS config ────────────────────────────────────────────
    // All mbedTLS structs are heap-allocated to avoid large stack frames.
    let mut entropy = Box::new(mbedtls_entropy_context::default());
    let mut drbg = Box::new(mbedtls_ctr_drbg_context::default());
    let mut conf = Box::new(mbedtls_ssl_config::default());

    // SAFETY: All pointers come from Box::as_mut() and are therefore valid,
    // aligned, and exclusively owned.  The init/seed/defaults/psk calls follow
    // the mbedTLS API contract documented in ssl.h and ctr_drbg.h.
    unsafe {
        mbedtls_entropy_init(entropy.as_mut());
        mbedtls_ctr_drbg_init(drbg.as_mut());

        let seed_label = b"petfilter-rpc";
        let rc = mbedtls_ctr_drbg_seed(
            drbg.as_mut(),
            Some(mbedtls_entropy_func),
            entropy.as_mut() as *mut _ as *mut core::ffi::c_void,
            seed_label.as_ptr(),
            seed_label.len(),
        );
        if rc != 0 {
            lwip_close(listener_fd);
            warn!("TLS(espidf): ctr_drbg_seed failed (rc={})", rc);
            return Err(TlsTransportError::Tls);
        }

        mbedtls_ssl_config_init(conf.as_mut());

        let rc = mbedtls_ssl_config_defaults(
            conf.as_mut(),
            MBEDTLS_SSL_IS_SERVER as _,
            MBEDTLS_SSL_TRANSPORT_STREAM as _,
            MBEDTLS_SSL_PRESET_DEFAULT as _,
        );
        if rc != 0 {
            lwip_close(listener_fd);
            warn!("TLS(espidf): ssl_config_defaults failed (rc={})", rc);
            return Err(TlsTransportError::Tls);
        }

        // No certificate required — PSK handles both auth and encryption.
        mbedtls_ssl_conf_authmode(conf.as_mut(), MBEDTLS_SSL_VERIFY_NONE as _);

        // Wire CSPRNG.
        mbedtls_ssl_conf_rng(
            conf.as_mut(),
            Some(mbedtls_ctr_drbg_random),
            drbg.as_mut() as *mut _ as *mut core::ffi::c_void,
        );

        // Restrict to pure-PSK key exchange for TLS 1.3.
        mbedtls_ssl_conf_tls13_key_exchange_modes(
            conf.as_mut(),
            MBEDTLS_SSL_TLS1_3_KEY_EXCHANGE_MODE_PSK as _,
        );

        // Register PSK with a well-known identity label.
        let rc = mbedtls_ssl_conf_psk(
            conf.as_mut(),
            psk.as_ptr(),
            psk.len(),
            PSK_IDENTITY.as_ptr(),
            PSK_IDENTITY.len(),
        );
        if rc != 0 {
            lwip_close(listener_fd);
            warn!("TLS(espidf): ssl_conf_psk failed (rc={})", rc);
            return Err(TlsTransportError::Tls);
        }
    }

    info!("TLS(espidf): listening on port {}", port);

    Ok(EspTlsServer {
        listener_fd,
        conf,
        entropy,
        drbg,
    })
}

/// Non-blocking accept + TLS 1.3 PSK handshake.
///
/// Returns `Some(EspTlsClient)` on a successful handshake; `None` if no
/// client is waiting or the handshake fails.
pub(super) fn esp_accept(server: &EspTlsServer) -> Option<EspTlsClient> {
    // Non-blocking accept — returns EAGAIN immediately if no client.
    // SAFETY: listener_fd is valid and non-blocking.
    let client_fd = unsafe {
        lwip_accept(
            server.listener_fd,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
        )
    };
    if client_fd < 0 {
        return None; // EAGAIN — no client waiting
    }

    // Set client socket non-blocking for the handshake loop.
    // SAFETY: F_SETFL is valid on a connected socket.
    let rc = unsafe { lwip_fcntl(client_fd, F_SETFL as _, O_NONBLOCK as _) };
    if rc < 0 {
        unsafe {
            lwip_close(client_fd);
        }
        warn!("TLS(espidf): O_NONBLOCK on client failed ({})", rc);
        return None;
    }

    // Allocate and initialise a TLS session context.
    let mut ssl = Box::new(mbedtls_ssl_context::default());

    // SAFETY: ssl is zero-initialised (Default); conf lifetime >= this session
    // because EspTlsServer outlives EspTlsClient in TlsTransport.
    unsafe {
        mbedtls_ssl_init(ssl.as_mut());

        let rc = mbedtls_ssl_setup(ssl.as_mut(), server.conf.as_ref());
        if rc != 0 {
            lwip_close(client_fd);
            warn!("TLS(espidf): ssl_setup failed (rc={})", rc);
            return None;
        }

        // Set BIO callbacks; client_fd is cast to void* as the context.
        // SAFETY: client_fd is a valid socket for the entire session, and
        // bio_send/bio_recv are correct unsafe extern "C" callbacks.
        mbedtls_ssl_set_bio(
            ssl.as_mut(),
            client_fd as usize as *mut core::ffi::c_void,
            Some(bio_send),
            Some(bio_recv),
            None,
        );
    }

    // Non-blocking handshake loop — retries on WANT_READ / WANT_WRITE.
    let mut retries = 0u32;
    loop {
        // SAFETY: ssl is initialised, BIO callbacks set, conf valid.
        let rc = unsafe { mbedtls_ssl_handshake(ssl.as_mut()) };

        if rc == 0 {
            break; // Handshake complete
        }
        if rc == MBEDTLS_ERR_SSL_WANT_READ || rc == MBEDTLS_ERR_SSL_WANT_WRITE {
            retries += 1;
            if retries >= HANDSHAKE_MAX_RETRIES {
                unsafe {
                    lwip_close(client_fd);
                }
                warn!("TLS(espidf): handshake timed out after {} retries", retries);
                return None;
            }
            // Yield 1 FreeRTOS tick to allow lwIP to receive in-flight data.
            // SAFETY: vTaskDelay is safe to call from any task context.
            unsafe { vTaskDelay(1) };
            continue;
        }
        // Fatal handshake error (e.g. wrong PSK, protocol error)
        unsafe {
            lwip_close(client_fd);
        }
        warn!("TLS(espidf): handshake failed (rc={})", rc);
        return None;
    }

    info!(
        "TLS(espidf): TLS 1.3 PSK handshake complete (fd={})",
        client_fd
    );
    Some(EspTlsClient { fd: client_fd, ssl })
}

/// Non-blocking read from an active TLS session.
///
/// Returns `Ok(0)` when no data is available (WANT_READ), `Ok(n)` for `n`
/// bytes read, or an error.
pub(super) fn esp_read(
    client: &mut EspTlsClient,
    buf: &mut [u8],
) -> Result<usize, TlsTransportError> {
    // SAFETY: ssl is valid and Connected; buf is a valid mutable slice.
    let rc = unsafe { mbedtls_ssl_read(client.ssl.as_mut(), buf.as_mut_ptr(), buf.len()) };

    if rc > 0 {
        return Ok(rc as usize);
    }
    if rc == MBEDTLS_ERR_SSL_WANT_READ {
        return Ok(0); // No data available — non-blocking
    }
    if rc == MBEDTLS_ERR_SSL_PEER_CLOSE_NOTIFY || rc == 0 {
        warn!("TLS(espidf): peer closed the connection");
        return Err(TlsTransportError::NotConnected);
    }
    warn!("TLS(espidf): ssl_read error (rc={})", rc);
    Err(TlsTransportError::Tls)
}

/// Non-blocking write to an active TLS session.
///
/// Returns `Ok(0)` when the output buffer is full (WANT_WRITE — caller
/// should retry), `Ok(n)` for `n` bytes written, or an error.
pub(super) fn esp_write(
    client: &mut EspTlsClient,
    data: &[u8],
) -> Result<usize, TlsTransportError> {
    // SAFETY: ssl is valid and Connected; data is a valid immutable slice.
    let rc = unsafe { mbedtls_ssl_write(client.ssl.as_mut(), data.as_ptr(), data.len()) };

    if rc > 0 {
        return Ok(rc as usize);
    }
    if rc == MBEDTLS_ERR_SSL_WANT_WRITE {
        return Ok(0); // Buffer full — retry
    }
    warn!("TLS(espidf): ssl_write error (rc={})", rc);
    Err(TlsTransportError::Tls)
}

// ── X.509 certificate support (Phase 3) ──────────────────────────────────────

/// Additional mbedTLS symbols needed for X.509 certificate auth.
use esp_idf_svc::sys::{
    mbedtls_pk_context, mbedtls_pk_free, mbedtls_pk_init, mbedtls_pk_parse_key,
    mbedtls_ssl_conf_ca_chain, mbedtls_ssl_conf_own_cert, mbedtls_x509_crt, mbedtls_x509_crt_free,
    mbedtls_x509_crt_init, mbedtls_x509_crt_parse,
};

/// Loaded X.509 certificate material for the TLS server.
///
/// Kept alive as long as the TLS server is running; mbedTLS references
/// these structures from the ssl_config.
pub(super) struct X509Context {
    pub(super) server_cert: Box<mbedtls_x509_crt>,
    pub(super) ca_cert: Box<mbedtls_x509_crt>,
    pub(super) server_key: Box<mbedtls_pk_context>,
}

impl Drop for X509Context {
    fn drop(&mut self) {
        // SAFETY: All fields were initialised by mbedtls_x509_crt_init / mbedtls_pk_init
        // and are freed exactly once.
        unsafe {
            mbedtls_x509_crt_free(self.server_cert.as_mut());
            mbedtls_x509_crt_free(self.ca_cert.as_mut());
            mbedtls_pk_free(self.server_key.as_mut());
        }
    }
}

/// Load X.509 certificates into the TLS server config.
///
/// `server_cert_pem`, `server_key_pem`, and `ca_cert_pem` must be
/// PEM-encoded and NUL-terminated (mbedTLS requirement for _parse functions).
///
/// Returns an `X509Context` that must be kept alive for the lifetime
/// of the TLS config.
///
/// # Safety
///
/// `conf` must be a valid, initialised `mbedtls_ssl_config`.
pub(super) unsafe fn esp_configure_x509(
    conf: &mut mbedtls_ssl_config,
    server_cert_pem: &[u8],
    server_key_pem: &[u8],
    ca_cert_pem: &[u8],
) -> Result<X509Context, super::TlsTransportError> {
    let mut server_cert = Box::new(mbedtls_x509_crt::default());
    let mut ca_cert = Box::new(mbedtls_x509_crt::default());
    let mut server_key = Box::new(mbedtls_pk_context::default());

    // SAFETY: All pointers are from Box::as_mut(), valid and exclusively owned.
    unsafe {
        mbedtls_x509_crt_init(server_cert.as_mut());
        mbedtls_x509_crt_init(ca_cert.as_mut());
        mbedtls_pk_init(server_key.as_mut());

        let rc = mbedtls_x509_crt_parse(
            server_cert.as_mut(),
            server_cert_pem.as_ptr(),
            server_cert_pem.len(),
        );
        if rc != 0 {
            warn!("TLS(espidf): x509_crt_parse(server) failed (rc={})", rc);
            return Err(super::TlsTransportError::Tls);
        }

        let rc = mbedtls_x509_crt_parse(ca_cert.as_mut(), ca_cert_pem.as_ptr(), ca_cert_pem.len());
        if rc != 0 {
            warn!("TLS(espidf): x509_crt_parse(ca) failed (rc={})", rc);
            return Err(super::TlsTransportError::Tls);
        }

        let rc = mbedtls_pk_parse_key(
            server_key.as_mut(),
            server_key_pem.as_ptr(),
            server_key_pem.len(),
            core::ptr::null(),
            0,
            None,
            core::ptr::null_mut(),
        );
        if rc != 0 {
            warn!("TLS(espidf): pk_parse_key failed (rc={})", rc);
            return Err(super::TlsTransportError::Tls);
        }

        // Register server certificate + key with the TLS config.
        let rc = mbedtls_ssl_conf_own_cert(conf, server_cert.as_mut(), server_key.as_mut());
        if rc != 0 {
            warn!("TLS(espidf): ssl_conf_own_cert failed (rc={})", rc);
            return Err(super::TlsTransportError::Tls);
        }

        // Register the CA chain for client certificate verification.
        mbedtls_ssl_conf_ca_chain(conf, ca_cert.as_mut(), core::ptr::null_mut());
    }

    info!("TLS(espidf): X.509 certificate auth configured");
    Ok(X509Context {
        server_cert,
        ca_cert,
        server_key,
    })
}
