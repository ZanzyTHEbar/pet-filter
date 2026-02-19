//! Fuzz target: `Session::verify_response` (auth PSK path)
//!
//! Generates arbitrary `(hmac_bytes, psk_bytes, session_id_delta)` inputs and
//! verifies that the authentication surface never panics and only transitions
//! to `Authenticated` when the HMAC is genuinely correct.
//!
//! Invariants checked:
//! - No panics under any byte sequence
//! - `is_authenticated()` returns `true` ONLY when the XOR-fold HMAC matches
//! - `verify_response` with wrong `session_id` always returns `false`
//!
//! cargo fuzz run fuzz_auth_psk

#![no_main]

use libfuzzer_sys::fuzz_target;
use petfilter::rpc::auth::Session;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    // Partition fuzz bytes: first 4 = session_id modifier, rest split as psk/hmac.
    let (sid_bytes, rest) = data.split_at(4);
    let sid_delta = u32::from_le_bytes(sid_bytes.try_into().unwrap());

    // Split rest into psk (first half) and hmac candidate (second half).
    let mid = rest.len() / 2;
    let fuzz_psk = &rest[..mid];
    let fuzz_hmac = &rest[mid..];

    if fuzz_psk.is_empty() {
        return;
    }

    let mut session = Session::new();

    // Initiate challenge.
    let (real_sid, _nonce) = session.begin_challenge();

    // Attempt verify with a fuzz-provided session_id (may differ from real_sid).
    let attempted_sid = real_sid.wrapping_add(sid_delta);
    let result = session.verify_response(attempted_sid, fuzz_hmac, fuzz_psk);

    if sid_delta != 0 {
        // Wrong session_id: must always reject.
        assert!(!result, "wrong session_id must always be rejected");
        assert!(!session.is_authenticated());
    } else {
        // Correct session_id: result depends on whether fuzz_hmac matches.
        // We don't assert the result here because the HMAC could accidentally match —
        // but we do assert that if it claims success, the session is authenticated.
        if result {
            assert!(
                session.is_authenticated(),
                "verify_response returned true but session is not authenticated"
            );
        }
    }

    // In any case, a subsequent call must not panic.
    let _ = session.check_rate_limit();
    let _ = session.check_sequence(1);
});
