//! RPC session authentication — challenge-response with HMAC-SHA256.
//!
//! Implements a lightweight PSK-based authentication handshake:
//!
//! 1. Client sends `AuthChallengeRequest`
//! 2. Device replies with `AuthChallengeResponse { nonce, session_id }`
//! 3. Client computes `HMAC-SHA256(psk, nonce)` and sends `AuthVerifyRequest`
//! 4. Device verifies the HMAC and transitions to `Authenticated`
//!
//! Platform-specific crypto is cfg-gated:
//! - ESP-IDF: `esp_fill_random` + mbedtls HMAC
//! - Simulation: deterministic stubs for host-side testing

use log::warn;

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
    rate_limiter: RateLimiter,
    next_session_id: u32,
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: SessionState::Unauthenticated,
            created_at: 0,
            rate_limiter: RateLimiter::new(10.0, 1.0),
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
    /// Transitions to `Authenticated` on success; resets to
    /// `Unauthenticated` on failure.
    pub fn verify_response(&mut self, session_id: u32, hmac: &[u8], psk: &[u8]) -> bool {
        let (expected_session_id, nonce) = match &self.state {
            SessionState::Challenged { nonce, session_id } => (*session_id, *nonce),
            _ => {
                warn!("auth: verify_response called outside Challenged state");
                return false;
            }
        };

        if session_id != expected_session_id {
            warn!("auth: session_id mismatch (got {session_id}, expected {expected_session_id})");
            self.reset();
            return false;
        }

        if !verify_hmac_sha256(psk, &nonce, hmac) {
            warn!("auth: HMAC verification failed");
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
        self.rate_limiter.try_consume()
    }

    /// Refill rate-limit tokens based on elapsed wall-clock time.
    pub fn refill_rate_limit(&mut self, elapsed_secs: f32) {
        self.rate_limiter.refill(elapsed_secs);
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

// ── Token-bucket rate limiter ────────────────────────────────

/// Simple token-bucket rate limiter.
///
/// Sustains `refill_rate` tokens per second up to `capacity` burst.
/// Default configuration: 10-token burst, 1 token/s refill (60/min).
pub struct RateLimiter {
    tokens: f32,
    capacity: f32,
    refill_rate: f32,
}

impl RateLimiter {
    pub fn new(capacity: f32, refill_rate: f32) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
        }
    }

    /// Try to consume a single token. Returns `true` if permitted.
    pub fn try_consume(&mut self) -> bool {
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            return true;
        }
        false
    }

    /// Add tokens proportional to elapsed wall-clock time.
    pub fn refill(&mut self, elapsed_secs: f32) {
        self.tokens = (self.tokens + self.refill_rate * elapsed_secs).min(self.capacity);
    }
}

// ── Platform-specific crypto ─────────────────────────────────

/// Fill a 32-byte nonce with cryptographically random data.
///
/// ESP-IDF: delegates to the hardware RNG via `esp_fill_random`.
#[cfg(target_os = "espidf")]
fn fill_random_nonce() -> [u8; 32] {
    let mut buf = [0u8; 32];
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

/// Verify `HMAC-SHA256(key, nonce)` against the supplied `tag`.
///
/// ESP-IDF: uses mbedtls `md_hmac` for a constant-time comparison.
#[cfg(target_os = "espidf")]
fn verify_hmac_sha256(key: &[u8], nonce: &[u8; 32], tag: &[u8]) -> bool {
    if tag.len() != 32 {
        return false;
    }

    let mut output = [0u8; 32];
    let md_info = unsafe {
        esp_idf_sys::mbedtls_md_info_from_type(esp_idf_sys::mbedtls_md_type_t_MBEDTLS_MD_SHA256)
    };
    if md_info.is_null() {
        warn!("auth: mbedtls SHA256 md_info unavailable");
        return false;
    }

    let rc = unsafe {
        esp_idf_sys::mbedtls_md_hmac(
            md_info,
            key.as_ptr(),
            key.len(),
            nonce.as_ptr(),
            nonce.len(),
            output.as_mut_ptr(),
        )
    };
    if rc != 0 {
        warn!("auth: mbedtls_md_hmac returned {rc}");
        return false;
    }

    constant_time_eq(&output, tag)
}

/// Simulation stub — recomputes HMAC with a naive XOR-fold for testing only.
///
/// NOT cryptographically secure; purely for host-side integration tests.
#[cfg(not(target_os = "espidf"))]
fn verify_hmac_sha256(key: &[u8], nonce: &[u8; 32], tag: &[u8]) -> bool {
    if tag.len() != 32 {
        return false;
    }
    let expected = sim_hmac(key, nonce);
    constant_time_eq(&expected, tag)
}

/// Constant-time comparison of two equal-length byte slices.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/// Simulation-only HMAC: XOR-fold key and nonce into a 32-byte tag.
///
/// Deterministic and simple enough for round-trip tests; provides zero
/// cryptographic guarantees.
#[cfg(not(target_os = "espidf"))]
fn sim_hmac(key: &[u8], nonce: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in nonce.iter().enumerate() {
        out[i] = b ^ key[i % key.len()];
    }
    out
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

        let tag = sim_hmac(psk, &nonce);
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
        let tag = sim_hmac(psk, &nonce);

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
    fn rate_limiter_basic() {
        let mut rl = RateLimiter::new(3.0, 1.0);

        assert!(rl.try_consume());
        assert!(rl.try_consume());
        assert!(rl.try_consume());
        assert!(!rl.try_consume()); // exhausted

        rl.refill(2.0);
        assert!(rl.try_consume());
        assert!(rl.try_consume());
        assert!(!rl.try_consume());
    }

    #[test]
    fn rate_limiter_does_not_exceed_capacity() {
        let mut rl = RateLimiter::new(5.0, 10.0);
        rl.tokens = 0.0;
        rl.refill(100.0);
        assert_eq!(rl.tokens, 5.0);
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
}
