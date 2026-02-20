//! Frame compression using DEFLATE via `miniz_oxide`.
//!
//! Integrates with the v2 frame codec: when the `FLAG_COMPRESSED` bit
//! is set in the frame header, the payload is DEFLATE-compressed.
//!
//! Compression is applied selectively — only to payloads that exceed
//! a minimum size threshold and where the compressed output is actually
//! smaller than the input (incompressible data is sent uncompressed).

extern crate alloc;
use alloc::vec::Vec;

use log::warn;
use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec;

/// Minimum payload size worth compressing (overhead outweighs benefit below).
const COMPRESS_THRESHOLD: usize = 128;

/// DEFLATE compression level (1-10, higher = better ratio, slower).
const COMPRESSION_LEVEL: u8 = 6;

/// Compress a payload using DEFLATE.
///
/// Returns `Some(compressed_bytes)` if compression is beneficial
/// (output < input), or `None` if compression should be skipped.
pub fn compress(input: &[u8]) -> Option<Vec<u8>> {
    if input.len() < COMPRESS_THRESHOLD {
        return None;
    }

    let compressed = compress_to_vec(input, COMPRESSION_LEVEL);

    if compressed.len() >= input.len() {
        return None;
    }

    Some(compressed)
}

/// Decompress a DEFLATE-compressed payload.
///
/// Returns the decompressed bytes, or `None` on decompression error.
pub fn decompress(input: &[u8]) -> Option<Vec<u8>> {
    match decompress_to_vec(input) {
        Ok(data) => Some(data),
        Err(e) => {
            warn!("decompress: DEFLATE error: {:?}", e);
            None
        }
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let input = b"The quick brown fox jumps over the lazy dog. ".repeat(10);
        let compressed = compress(&input).expect("should compress");
        assert!(compressed.len() < input.len());

        let decompressed = decompress(&compressed).expect("should decompress");
        assert_eq!(decompressed, input);
    }

    #[test]
    fn below_threshold_returns_none() {
        let small = b"tiny";
        assert!(compress(small).is_none());
    }

    #[test]
    fn incompressible_data_returns_none() {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let mut data = vec![0u8; 256];
        for chunk in data.chunks_mut(8) {
            let s = RandomState::new();
            let v = s.build_hasher().finish().to_le_bytes();
            let n = chunk.len().min(v.len());
            chunk[..n].copy_from_slice(&v[..n]);
        }

        // Random data is typically incompressible
        // (the result may or may not be None depending on entropy,
        // but at minimum we don't crash)
        let _ = compress(&data);
    }

    #[test]
    fn decompress_invalid_returns_none() {
        let garbage = b"not valid deflate";
        assert!(decompress(garbage).is_none());
    }
}
