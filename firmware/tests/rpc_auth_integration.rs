//! Integration tests: RPC auth handshake + rate limiting (QA-7d).

use petfilter::rpc::auth::Session;

#[test]
fn fresh_session_is_unauthenticated() {
    let session = Session::new();
    assert!(!session.is_authenticated());
}

#[test]
fn sequence_check_requires_authenticated_state() {
    let mut session = Session::new();
    // Before auth, check_sequence always returns false.
    assert!(!session.check_sequence(1));
}

#[test]
fn authenticated_session_enforces_monotonic_sequence() {
    let mut session = Session::new();
    let (session_id, nonce) = session.begin_challenge();

    // Compute the expected HMAC (sim path: XOR of psk with nonce bytes).
    let psk = b"test-psk";
    let mut expected = [0u8; 32];
    for i in 0..32 {
        expected[i] = psk[i % psk.len()] ^ nonce[i];
    }
    assert!(session.verify_response(session_id, &expected, psk));
    assert!(session.is_authenticated());

    assert!(session.check_sequence(1));
    assert!(session.check_sequence(2));
    assert!(!session.check_sequence(2), "replay rejected");
    assert!(!session.check_sequence(1), "regression rejected");
    assert!(session.check_sequence(100), "forward jump accepted");
}

#[test]
fn rate_limit_exhaustion_then_refill() {
    let mut session = Session::new();
    let mut rejected = 0;
    for _ in 0..120 {
        if !session.check_rate_limit() {
            rejected += 1;
        }
    }
    assert!(rejected > 0, "rate limit must reject once bucket is exhausted");

    session.refill_rate_limit(2.0);
    assert!(session.check_rate_limit(), "after refill, requests accepted");
}

#[test]
fn wrong_hmac_rejects_auth() {
    let mut session = Session::new();
    let (session_id, _nonce) = session.begin_challenge();
    assert!(!session.verify_response(session_id, &[0xDEu8; 32], b"test-psk"));
    assert!(!session.is_authenticated());
}

#[test]
fn session_reset_clears_auth() {
    let mut session = Session::new();
    let (session_id, nonce) = session.begin_challenge();
    let psk = b"test-psk";
    let mut hmac = [0u8; 32];
    for i in 0..32 { hmac[i] = psk[i % psk.len()] ^ nonce[i]; }
    session.verify_response(session_id, &hmac, psk);
    assert!(session.is_authenticated());

    session.reset();
    assert!(!session.is_authenticated(), "reset must clear authenticated state");
    assert!(!session.check_sequence(1), "sequence check fails after reset");
}
