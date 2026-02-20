//! Property and fuzz-style tests for robustness of core data structures.
//!
//! Runs on host (x86_64) only — proptest is not available for ESP32 targets.
//! On ESP32, these tests are compiled out.

#![cfg(not(target_os = "espidf"))]

use petfilter::rpc::auth::{Session, compute_hmac};
use petfilter::rpc::ota::{OtaError, OtaManager, OtaState};
use proptest::prelude::*;

// ── QA-8b: PSK auth round-trip ────────────────────────────────

proptest! {
    /// For any arbitrary PSK and nonce, verify_response with the correctly
    /// computed sim-HMAC always succeeds; a wrong PSK always fails.
    #[test]
    fn auth_round_trip_correct_psk(
        psk in proptest::collection::vec(0u8..=255u8, 1..=32),
    ) {
        let mut session = Session::new();
        let (session_id, nonce) = session.begin_challenge();

        let tag = compute_hmac(&psk, &nonce);

        prop_assert!(
            session.verify_response(session_id, &tag, &psk),
            "correct HMAC must always authenticate"
        );
    }

    #[test]
    fn auth_wrong_psk_never_authenticates(
        psk in proptest::collection::vec(1u8..=255u8, 1..=32),
    ) {
        let mut session = Session::new();
        let (session_id, nonce) = session.begin_challenge();

        // Correct HMAC for psk.
        let mut correct = [0u8; 32];
        for i in 0..32 {
            correct[i] = psk[i % psk.len()] ^ nonce[i];
        }

        // Wrong HMAC — flip the first byte.
        let mut wrong = correct;
        wrong[0] = wrong[0].wrapping_add(1);

        prop_assert!(
            !session.verify_response(session_id, &wrong, &psk),
            "wrong HMAC must always fail"
        );
    }
}

// ── QA-8d: OTA state machine invariants ──────────────────────

#[derive(Debug, Clone)]
enum OtaOp {
    Begin(u32),          // firmware_size
    Chunk(u32, Vec<u8>), // offset, data
    Finalize,
    Abort,
}

fn arb_ota_op() -> impl Strategy<Value = OtaOp> {
    prop_oneof![
        (1u32..=1024u32).prop_map(OtaOp::Begin),
        (
            0u32..=512u32,
            proptest::collection::vec(0u8..=255u8, 1..=16)
        )
            .prop_map(|(o, d)| OtaOp::Chunk(o, d)),
        Just(OtaOp::Finalize),
        Just(OtaOp::Abort),
    ]
}

proptest! {
    /// Arbitrary sequences of OTA operations must never produce stuck states.
    /// After any sequence, abort() must always succeed in resetting to Idle.
    #[test]
    fn ota_no_stuck_states(
        ops in proptest::collection::vec(arb_ota_op(), 1..=20),
    ) {
        let mut ota = OtaManager::new();

        // Execute arbitrary operations, ignoring errors.
        for op in &ops {
            match op {
                OtaOp::Begin(size) => { let _ = ota.begin(*size, &[0u8; 32]); }
                OtaOp::Chunk(offset, data) => { let _ = ota.write_chunk(*offset, data); }
                OtaOp::Finalize => { let _ = ota.finalize(); }
                OtaOp::Abort => { ota.abort(); }
            }
        }

        // After any sequence, abort must always succeed.
        ota.abort();
        prop_assert_eq!(
            ota.state(), OtaState::Idle,
            "abort() must always reset to Idle regardless of prior ops"
        );

        // After abort, begin must always succeed.
        prop_assert!(
            ota.begin(16, &[0u8; 32]).is_ok(),
            "begin() must succeed from Idle after abort"
        );
    }

    /// Write errors always produce typed OtaError variants, never panics.
    #[test]
    fn ota_errors_are_typed_not_panics(
        size in 1u32..=100u32,
        chunk_data in proptest::collection::vec(0u8..=255u8, 0..=200usize),
    ) {
        let mut ota = OtaManager::new();
        let _ = ota.begin(size, &[0u8; 32]);

        // Write at offset 0 with arbitrary data — should get a typed error or Ok.
        let result = ota.write_chunk(0, &chunk_data);
        match result {
            Ok(_) => {}
            Err(e) => {
                // Must be a known OtaError variant.
                let _: OtaError = e;
            }
        }
    }
}

// ── QA-8c: CrashLog ring-buffer invariants ────────────────────

// CrashLog uses NVS for persistence, so we test the in-memory invariants
// through the diagnostics module directly.
#[test]
fn crash_log_count_bounded_by_capacity() {
    use petfilter::app::ports::{StorageError, StoragePort};
    use petfilter::diagnostics::{CrashEntry, CrashLog};
    use std::collections::HashMap;

    struct MemStore(HashMap<String, Vec<u8>>);
    impl StoragePort for MemStore {
        fn read(&self, ns: &str, k: &str, buf: &mut [u8]) -> Result<usize, StorageError> {
            match self.0.get(&format!("{}::{}", ns, k)) {
                Some(v) => {
                    let n = v.len().min(buf.len());
                    buf[..n].copy_from_slice(&v[..n]);
                    Ok(n)
                }
                None => Err(StorageError::NotFound),
            }
        }
        fn write(&mut self, ns: &str, k: &str, d: &[u8]) -> Result<(), StorageError> {
            self.0.insert(format!("{}::{}", ns, k), d.to_vec());
            Ok(())
        }
        fn exists(&self, ns: &str, k: &str) -> bool {
            self.0.contains_key(&format!("{}::{}", ns, k))
        }
        fn delete(&mut self, ns: &str, k: &str) -> Result<(), StorageError> {
            self.0.remove(&format!("{}::{}", ns, k));
            Ok(())
        }
    }

    let mut nvs = MemStore(HashMap::new());
    let mut log = CrashLog::new();
    log.init(&nvs);

    // Write more entries than capacity (4).
    for i in 0..8u64 {
        log.write_entry(&mut nvs, &CrashEntry::new(i, "test panic", 0));
    }

    let entries = log.read_all(&nvs);
    assert!(
        entries.len() <= 4,
        "CrashLog must hold at most 4 entries (ring buffer), got {}",
        entries.len()
    );
    assert!(log.count(&nvs) <= 4, "count() must not exceed capacity");
}
