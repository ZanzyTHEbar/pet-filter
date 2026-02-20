//! RPC session authentication — challenge-response with HMAC-SHA256.
//!
//! Implements a lightweight PSK-based authentication handshake:
//!
//! 1. Client sends `AuthChallengeRequest`
//! 2. Device replies with `AuthChallengeResponse { nonce, session_id }`
//! 3. Client computes `HMAC-SHA256(psk, nonce)` and sends `AuthVerifyRequest`
//! 4. Device verifies the HMAC and transitions to `Authenticated`
//!
//! Crypto is handled by the `hmac-sha256` crate — pure Rust, no_std,
//! constant-time verification, identical on ESP-IDF and host targets.

use burster::Limiter;
use core::time::Duration;
use log::warn;

// ── Constants ────────────────────────────────────────────────

/// Maximum number of concurrent RPC client sessions.
pub const MAX_CLIENTS: usize = 3;

/// Client identifier (index into the session table).
pub type ClientId = u8;

// ── Session state machine ────────────────────────────────────

/// Authentication state of a single RPC session.
#[derive(Debug, Clone)]
pub enum SessionState {
    Unauthenticated,
    Challenged { nonce: [u8; 32], session_id: u32 },
    Authenticated { session_id: u32, msg_seq: u32 },
}

/// Tracks a single client session through the auth handshake and beyond.
pub struct Session {
    pub state: SessionState,
    pub created_at: u64,
    rate_limiter: burster::TokenBucket<fn() -> Duration>,
    next_session_id: u32,
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: SessionState::Unauthenticated,
            created_at: 0,
            rate_limiter: burster::TokenBucket::new_with_time_provider(
                10,
                10, // 10 tokens per second, 10 burst capacity
                platform_now as fn() -> Duration,
            ),
            next_session_id: 1,
        }
    }

    /// Begin the challenge-response handshake.
    ///
    /// Returns `(session_id, nonce)` for inclusion in `AuthChallengeResponse`.
    pub fn begin_challenge(&mut self) -> (u32, [u8; 32]) {
        let session_id = self.alloc_session_id();
        let nonce = fill_random_nonce();

        self.state = SessionState::Challenged { nonce, session_id };
        (session_id, nonce)
    }

    /// Verify the client's HMAC response against the stored nonce and PSK.
    ///
    /// Uses `hmac_sha256::HMAC` with constant-time verification.
    /// Transitions to `Authenticated` on success; resets to
    /// `Unauthenticated` on failure.
    pub fn verify_response(&mut self, session_id: u32, hmac_tag: &[u8], psk: &[u8]) -> bool {
        let (expected_session_id, nonce) =
            if let SessionState::Challenged { nonce, session_id } = &self.state {
                (*session_id, *nonce)
            } else {
                warn!("auth: verify_response called outside Challenged state");
                return false;
            };

        if session_id != expected_session_id {
            warn!("auth: session_id mismatch (got {session_id}, expected {expected_session_id})");
            self.reset();
            return false;
        }

        if hmac_tag.len() != 32 {
            warn!("auth: HMAC tag length invalid ({})", hmac_tag.len());
            self.reset();
            return false;
        }

        let computed = hmac_sha256::HMAC::mac(nonce, psk);
        let tag_array: &[u8; 32] = hmac_tag.try_into().unwrap();
        if !hmac_sha256::HMAC::verify(nonce, psk, tag_array) {
            warn!("auth: HMAC verification failed");
            let _ = computed; // prevent optimization of timing side-channel
            self.reset();
            return false;
        }

        self.state = SessionState::Authenticated {
            session_id,
            msg_seq: 0,
        };
        true
    }

    /// Validate that `msg_id` is strictly greater than the last seen sequence.
    pub fn check_sequence(&mut self, msg_id: u32) -> bool {
        match &mut self.state {
            SessionState::Authenticated { msg_seq, .. } => {
                if msg_id <= *msg_seq {
                    warn!("auth: sequence regression (got {msg_id}, last {msg_seq})");
                    return false;
                }
                *msg_seq = msg_id;
                true
            }
            _ => false,
        }
    }

    /// Consume one rate-limit token; returns `false` when exhausted.
    pub fn check_rate_limit(&mut self) -> bool {
        self.rate_limiter.try_consume(1).is_ok()
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self.state, SessionState::Authenticated { .. })
    }

    /// Reset the session back to `Unauthenticated`.
    pub fn reset(&mut self) {
        self.state = SessionState::Unauthenticated;
    }

    fn alloc_session_id(&mut self) -> u32 {
        let id = self.next_session_id;
        self.next_session_id = self.next_session_id.wrapping_add(1);
        id
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// ── Session table for multi-client support ───────────────────

/// Fixed-size table of per-client sessions.
///
/// Each slot maps to one connected RPC client. Slots are indexed
/// by `ClientId` (0..MAX_CLIENTS).
pub struct SessionTable {
    sessions: [Session; MAX_CLIENTS],
}

impl SessionTable {
    pub fn new() -> Self {
        Self {
            sessions: core::array::from_fn(|_| Session::new()),
        }
    }

    /// Get a mutable reference to the session for `client_id`.
    pub fn get_mut(&mut self, client_id: ClientId) -> Option<&mut Session> {
        self.sessions.get_mut(client_id as usize)
    }

    /// Get a shared reference to the session for `client_id`.
    pub fn get(&self, client_id: ClientId) -> Option<&Session> {
        self.sessions.get(client_id as usize)
    }

    /// Reset a specific client's session (e.g. on disconnect).
    pub fn reset_client(&mut self, client_id: ClientId) {
        if let Some(s) = self.sessions.get_mut(client_id as usize) {
            s.reset();
        }
    }

    /// Reset all sessions.
    pub fn reset_all(&mut self) {
        for s in &mut self.sessions {
            s.reset();
        }
    }

    /// Returns true if the specified client is authenticated.
    pub fn is_authenticated(&self, client_id: ClientId) -> bool {
        self.sessions
            .get(client_id as usize)
            .is_some_and(Session::is_authenticated)
    }
}

impl Default for SessionTable {
    fn default() -> Self {
        Self::new()
    }
}

// ── Compute HMAC for client-side (used in tests) ─────────────

/// Compute `HMAC-SHA256(psk, nonce)` — used by test code to simulate
/// the client side of the challenge-response handshake.
pub fn compute_hmac(psk: &[u8], nonce: &[u8; 32]) -> [u8; 32] {
    hmac_sha256::HMAC::mac(*nonce, psk)
}

// ── Platform-specific nonce generation ───────────────────────

/// Fill a 32-byte nonce with cryptographically random data.
///
/// ESP-IDF: delegates to the hardware RNG via `esp_fill_random`.
#[cfg(target_os = "espidf")]
fn fill_random_nonce() -> [u8; 32] {
    let mut buf = [0u8; 32];
    // SAFETY: esp_fill_random writes to the provided buffer using
    // the hardware RNG. Buffer is valid and exclusively owned.
    unsafe {
        esp_idf_sys::esp_fill_random(buf.as_mut_ptr().cast(), buf.len());
    }
    buf
}

/// Simulation stub — uses `RandomState` to produce non-cryptographic entropy.
#[cfg(not(target_os = "espidf"))]
fn fill_random_nonce() -> [u8; 32] {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut buf = [0u8; 32];
    for chunk in buf.chunks_mut(8) {
        let s = RandomState::new();
        let val = s.build_hasher().finish().to_le_bytes();
        let len = chunk.len().min(val.len());
        chunk[..len].copy_from_slice(&val[..len]);
    }
    buf
}

// ── Platform time for rate limiter ───────────────────────────

#[cfg(target_os = "espidf")]
fn platform_now() -> Duration {
    let us = unsafe { esp_idf_sys::esp_timer_get_time() };
    Duration::from_micros(us as u64)
}

#[cfg(not(target_os = "espidf"))]
fn platform_now() -> Duration {
    use std::time::Instant;
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(all(test, not(target_os = "espidf")))]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle_happy_path() {
        let psk = b"test-psk-12345";
        let mut sess = Session::new();

        assert!(!sess.is_authenticated());

        let (sid, nonce) = sess.begin_challenge();
        assert!(matches!(sess.state, SessionState::Challenged { .. }));

        let tag = compute_hmac(psk, &nonce);
        assert!(sess.verify_response(sid, &tag, psk));
        assert!(sess.is_authenticated());
    }

    #[test]
    fn session_rejects_wrong_hmac() {
        let psk = b"correct-key";
        let mut sess = Session::new();

        let (sid, _nonce) = sess.begin_challenge();
        let bad_tag = [0xFFu8; 32];

        assert!(!sess.verify_response(sid, &bad_tag, psk));
        assert!(!sess.is_authenticated());
    }

    #[test]
    fn session_rejects_wrong_session_id() {
        let psk = b"key";
        let mut sess = Session::new();

        let (sid, nonce) = sess.begin_challenge();
        let tag = compute_hmac(psk, &nonce);

        assert!(!sess.verify_response(sid + 999, &tag, psk));
        assert!(!sess.is_authenticated());
    }

    #[test]
    fn sequence_check_enforces_monotonic_increase() {
        let mut sess = Session::new();
        sess.state = SessionState::Authenticated {
            session_id: 1,
            msg_seq: 0,
        };

        assert!(sess.check_sequence(1));
        assert!(sess.check_sequence(2));
        assert!(sess.check_sequence(5));
        assert!(!sess.check_sequence(5)); // duplicate
        assert!(!sess.check_sequence(3)); // regression
        assert!(sess.check_sequence(6));
    }

    #[test]
    fn rate_limiter_exhaustion() {
        let mut sess = Session::new();
        for _ in 0..10 {
            assert!(sess.check_rate_limit());
        }
        assert!(!sess.check_rate_limit()); // 11th should be rejected
    }

    #[test]
    fn reset_returns_to_unauthenticated() {
        let mut sess = Session::new();
        sess.state = SessionState::Authenticated {
            session_id: 42,
            msg_seq: 10,
        };
        sess.reset();
        assert!(matches!(sess.state, SessionState::Unauthenticated));
    }

    #[test]
    fn session_table_multi_client() {
        let mut table = SessionTable::new();

        assert!(!table.is_authenticated(0));
        assert!(!table.is_authenticated(1));

        if let Some(s) = table.get_mut(0) {
            s.state = SessionState::Authenticated {
                session_id: 1,
                msg_seq: 0,
            };
        }
        assert!(table.is_authenticated(0));
        assert!(!table.is_authenticated(1));

        table.reset_client(0);
        assert!(!table.is_authenticated(0));
    }

    #[test]
    fn session_table_out_of_bounds() {
        let table = SessionTable::new();
        assert!(table.get(MAX_CLIENTS as u8).is_none());
        assert!(!table.is_authenticated(255));
    }

    #[test]
    fn compute_hmac_is_deterministic() {
        let psk = b"key";
        let nonce = [42u8; 32];
        let h1 = compute_hmac(psk, &nonce);
        let h2 = compute_hmac(psk, &nonce);
        assert_eq!(h1, h2);
    }
}
