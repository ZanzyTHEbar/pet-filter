//! Integration tests for RPC authentication and session logic.
//!
//! QA-7d: rate limit exhaustion → rejection.

use petfilter::rpc::auth::Session;

#[test]
fn fresh_session_is_not_authenticated() {
    let session = Session::new();
    assert!(!session.is_authenticated(), "new session must start unauthenticated");
}

#[test]
fn sequence_check_requires_authenticated_state() {
    // In Unauthenticated state, check_sequence always returns false
    // because the msg_seq is only valid after auth.
    let mut session = Session::new();
    assert!(
        !session.check_sequence(1),
        "check_sequence returns false when not authenticated"
    );
}

#[test]
fn authenticated_session_accepts_monotonic_ids() {
    let mut session = Session::new();
    // Manually force to authenticated state via challenge-response.
    let (session_id, nonce) = session.begin_challenge();
    // Compute sim_hmac manually (test-only path, same as auth.rs internals).
    let psk = b"test-psk";
    // XOR-based sim HMAC (from auth.rs non-espidf path).
    let mut expected = [0u8; 32];
    for i in 0..32 {
        expected[i] = psk[i % psk.len()] ^ nonce[i];
    }
    let authenticated = session.verify_response(session_id, &expected, psk);
    assert!(authenticated, "verify_response with correct HMAC should succeed");
    assert!(session.is_authenticated(), "session should be authenticated");

    // Now sequence checks work.
    assert!(session.check_sequence(1));
    assert!(session.check_sequence(2));
    assert!(!session.check_sequence(2), "replay of seq=2 rejected");
    assert!(!session.check_sequence(1), "regression rejected");
    assert!(session.check_sequence(100), "jump forward accepted");
}

#[test]
fn rate_limit_exhaustion_then_refill() {
    let mut session = Session::new();
    // Exhaust the token bucket (burst capacity is typically 10-60 tokens).
    let mut rejected = 0;
    for _ in 0..120 {
        if !session.check_rate_limit() {
            rejected += 1;
        }
    }
    assert!(rejected > 0, "rate limit should reject requests once bucket is empty");

    // Refill over 1 second.
    session.refill_rate_limit(1.0);
    assert!(
        session.check_rate_limit(),
        "after refill, at least one request should be accepted"
    );
}

#[test]
fn wrong_hmac_does_not_authenticate() {
    let mut session = Session::new();
    let (session_id, _nonce) = session.begin_challenge();
    let wrong_hmac = [0xDEu8; 32];
    assert!(
        !session.verify_response(session_id, &wrong_hmac, b"test-psk"),
        "wrong HMAC must not authenticate"
    );
    assert!(!session.is_authenticated());
}
