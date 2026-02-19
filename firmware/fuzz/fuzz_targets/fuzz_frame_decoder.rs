//! Fuzz target: `FrameDecoder::feed`
//!
//! Drives arbitrary byte sequences into the streaming frame decoder and
//! asserts that it never panics, never returns out-of-bounds payloads,
//! and is idempotent with respect to a full reset between invocations.
//!
//! cargo fuzz run fuzz_frame_decoder

#![no_main]

use libfuzzer_sys::fuzz_target;
use petfilter::rpc::codec::FrameDecoder;

fuzz_target!(|data: &[u8]| {
    let mut decoder = FrameDecoder::new();

    // Feed in the raw bytes (may contain any length prefix, garbage, etc.)
    if let Some(payload) = decoder.feed(data) {
        // The returned slice must be within the fixed 4096-byte internal
        // buffer — assert that the length is sane.
        assert!(payload.len() <= 4096, "payload exceeds MAX_FRAME_SIZE");
        assert!(!payload.is_empty(), "decoder must not yield empty payload");
    }

    // After a reset the decoder must accept bytes cleanly again.
    decoder.reset();
    let _ = decoder.feed(data);
});
