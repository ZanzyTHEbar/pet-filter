//! Chunked transfer encoding for large RPC payloads.
//!
//! When a response exceeds the maximum single-frame size (e.g. OTA
//! firmware data, large diagnostics responses), it is split into
//! multiple frames using the v2 codec's chunking flags.
//!
//! ```text
//! Frame 1: [FLAG_CHUNKED | FLAG_COMPRESSED?]  payload part 1
//! Frame 2: [FLAG_CHUNKED]                      payload part 2
//! Frame N: [FLAG_CHUNKED | FLAG_LAST_CHUNK]    payload part N (final)
//! ```
//!
//! The receiver reassembles chunks before passing the complete
//! payload to the RPC dispatcher.

extern crate alloc;
use alloc::vec::Vec;

use super::codec::{self, FLAG_CHUNKED, FLAG_LAST_CHUNK, FrameHeader};

/// Maximum chunk payload size (fits within a single frame).
const CHUNK_SIZE: usize = 2048;

/// Split a large payload into chunked frames.
///
/// Returns a vector of `(flags, payload_slice)` tuples ready for
/// encoding with `encode_frame_with_flags`.
pub fn chunk_payload(data: &[u8]) -> Vec<(u8, &[u8])> {
    if data.len() <= codec::MAX_FRAME_SIZE {
        return alloc::vec![(0, data)];
    }

    let chunks: Vec<&[u8]> = data.chunks(CHUNK_SIZE).collect();
    let total = chunks.len();

    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut flags = FLAG_CHUNKED;
            if i == total - 1 {
                flags |= FLAG_LAST_CHUNK;
            }
            (flags, chunk)
        })
        .collect()
}

/// Reassembly buffer for incoming chunked frames.
pub struct ChunkReassembler {
    buffer: Vec<u8>,
    active: bool,
}

impl ChunkReassembler {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            active: false,
        }
    }

    /// Feed a frame's payload and header into the reassembler.
    ///
    /// Returns `Some(complete_payload)` when the last chunk arrives.
    /// Returns `None` when more chunks are expected.
    pub fn feed(&mut self, header: FrameHeader, payload: &[u8]) -> Option<Vec<u8>> {
        if !header.is_chunked() {
            // Non-chunked frame — return as-is.
            self.reset();
            return Some(payload.to_vec());
        }

        if !self.active {
            self.buffer.clear();
            self.active = true;
        }

        self.buffer.extend_from_slice(payload);

        if header.is_last_chunk() {
            self.active = false;
            let complete = core::mem::take(&mut self.buffer);
            Some(complete)
        } else {
            None
        }
    }

    /// Reset the reassembler, discarding any partial data.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.active = false;
    }

    /// Whether reassembly is in progress.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for ChunkReassembler {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_payload_not_chunked() {
        let data = b"small payload";
        let chunks = chunk_payload(data);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0, 0); // no flags
        assert_eq!(chunks[0].1, data);
    }

    #[test]
    fn large_payload_chunked() {
        let data = vec![0xAA; 5000]; // > MAX_FRAME_SIZE
        let chunks = chunk_payload(&data);

        assert!(chunks.len() > 1);

        // All chunks except last should have FLAG_CHUNKED
        for (i, (flags, _)) in chunks.iter().enumerate() {
            assert!(*flags & FLAG_CHUNKED != 0);
            if i == chunks.len() - 1 {
                assert!(*flags & FLAG_LAST_CHUNK != 0);
            } else {
                assert!(*flags & FLAG_LAST_CHUNK == 0);
            }
        }

        // Total reassembled size matches
        let total: usize = chunks.iter().map(|(_, c)| c.len()).sum();
        assert_eq!(total, data.len());
    }

    #[test]
    fn reassembler_single_frame() {
        let mut r = ChunkReassembler::new();
        let hdr = FrameHeader { flags: 0 };
        let result = r.feed(hdr, b"complete");
        assert_eq!(result.unwrap(), b"complete");
    }

    #[test]
    fn reassembler_multi_chunk() {
        let mut r = ChunkReassembler::new();

        let h1 = FrameHeader {
            flags: FLAG_CHUNKED,
        };
        assert!(r.feed(h1, b"part1").is_none());
        assert!(r.is_active());

        let h2 = FrameHeader {
            flags: FLAG_CHUNKED,
        };
        assert!(r.feed(h2, b"part2").is_none());

        let h3 = FrameHeader {
            flags: FLAG_CHUNKED | FLAG_LAST_CHUNK,
        };
        let result = r.feed(h3, b"part3").unwrap();
        assert_eq!(result, b"part1part2part3");
        assert!(!r.is_active());
    }

    #[test]
    fn reassembler_reset() {
        let mut r = ChunkReassembler::new();
        let h = FrameHeader {
            flags: FLAG_CHUNKED,
        };
        r.feed(h, b"partial");
        assert!(r.is_active());

        r.reset();
        assert!(!r.is_active());
    }
}
