//! Length-prefix frame codec (v2 with flags byte).
//!
//! Wire format v2:
//! ```text
//! ┌────────────┬───────┬──────────────────────────┐
//! │ Length (4B) │ Flags │ FlatBuffer payload (N B) │
//! │ LE u32      │ (1B)  │                          │
//! └────────────┴───────┴──────────────────────────┘
//! ```
//!
//! v1 compatibility: frames where `flags == 0x00` are treated identically
//! to v1 frames (no compression, no chunking). The length field covers
//! both the flags byte and payload.
//!
//! ## Flags byte layout
//!
//! | Bit | Meaning                          |
//! |-----|----------------------------------|
//! | 0   | Compressed (deflate via miniz)   |
//! | 1   | Chunked (part of multi-frame)    |
//! | 2   | Last chunk in sequence           |
//! | 3-7 | Reserved (must be 0)             |

/// Maximum frame payload size (protects against memory exhaustion).
pub const MAX_FRAME_SIZE: usize = 4096;

/// Frame header size: 4-byte LE length + 1-byte flags.
const HEADER_SIZE: usize = 5;

/// v1 header size (no flags byte) for backward detection.
const V1_HEADER_SIZE: usize = 4;

// ── Flag bits ────────────────────────────────────────────────

/// Frame payload is DEFLATE-compressed.
pub const FLAG_COMPRESSED: u8 = 0x01;

/// Frame is a chunk in a multi-frame sequence.
pub const FLAG_CHUNKED: u8 = 0x02;

/// Last chunk in a chunked sequence.
pub const FLAG_LAST_CHUNK: u8 = 0x04;

/// Mask of all valid flag bits.
const VALID_FLAGS_MASK: u8 = FLAG_COMPRESSED | FLAG_CHUNKED | FLAG_LAST_CHUNK;

/// Parsed frame metadata returned alongside payload bytes.
#[derive(Debug, Clone, Copy)]
pub struct FrameHeader {
    pub flags: u8,
}

impl FrameHeader {
    pub fn is_compressed(self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    pub fn is_chunked(self) -> bool {
        self.flags & FLAG_CHUNKED != 0
    }

    pub fn is_last_chunk(self) -> bool {
        self.flags & FLAG_LAST_CHUNK != 0
    }
}

// ── Decoder ──────────────────────────────────────────────────

/// Decoder state machine.
enum DecoderState {
    ReadingHeader {
        collected: usize,
    },
    ReadingPayload {
        expected: usize,
        collected: usize,
        flags: u8,
    },
}

/// Streaming frame decoder (v2-aware, v1-compatible).
pub struct FrameDecoder {
    state: DecoderState,
    header_buf: [u8; HEADER_SIZE],
    payload_buf: [u8; MAX_FRAME_SIZE],
    last_header: FrameHeader,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            state: DecoderState::ReadingHeader { collected: 0 },
            header_buf: [0; HEADER_SIZE],
            payload_buf: [0; MAX_FRAME_SIZE],
            last_header: FrameHeader { flags: 0 },
        }
    }

    /// Feed bytes into the decoder.
    ///
    /// Returns `Some(&[u8])` when a complete frame payload is available.
    /// Use [`last_header()`] to inspect flags. The returned slice is
    /// valid until the next call to `feed`.
    pub fn feed(&mut self, data: &[u8]) -> Option<&[u8]> {
        let mut offset = 0;

        while offset < data.len() {
            match &mut self.state {
                DecoderState::ReadingHeader { collected } => {
                    let needed = HEADER_SIZE - *collected;
                    let available = data.len() - offset;
                    let to_copy = needed.min(available);

                    self.header_buf[*collected..*collected + to_copy]
                        .copy_from_slice(&data[offset..offset + to_copy]);

                    *collected += to_copy;
                    offset += to_copy;

                    if *collected == HEADER_SIZE {
                        let total_len = u32::from_le_bytes([
                            self.header_buf[0],
                            self.header_buf[1],
                            self.header_buf[2],
                            self.header_buf[3],
                        ]) as usize;

                        let flags = self.header_buf[4];

                        if total_len == 0 || total_len > MAX_FRAME_SIZE + 1 {
                            self.state = DecoderState::ReadingHeader { collected: 0 };
                            continue;
                        }

                        if flags & !VALID_FLAGS_MASK != 0 {
                            self.state = DecoderState::ReadingHeader { collected: 0 };
                            continue;
                        }

                        let payload_len = total_len.saturating_sub(1);
                        if payload_len > MAX_FRAME_SIZE {
                            self.state = DecoderState::ReadingHeader { collected: 0 };
                            continue;
                        }

                        self.state = DecoderState::ReadingPayload {
                            expected: payload_len,
                            collected: 0,
                            flags,
                        };
                    }
                }

                DecoderState::ReadingPayload {
                    expected,
                    collected,
                    flags,
                } => {
                    let needed = *expected - *collected;
                    let available = data.len() - offset;
                    let to_copy = needed.min(available);

                    self.payload_buf[*collected..*collected + to_copy]
                        .copy_from_slice(&data[offset..offset + to_copy]);

                    *collected += to_copy;
                    offset += to_copy;

                    if *collected == *expected {
                        let len = *expected;
                        let f = *flags;
                        self.last_header = FrameHeader { flags: f };
                        self.state = DecoderState::ReadingHeader { collected: 0 };
                        return Some(&self.payload_buf[..len]);
                    }
                }
            }
        }

        None
    }

    /// Returns the header (flags) of the most recently decoded frame.
    pub fn last_header(&self) -> FrameHeader {
        self.last_header
    }

    /// Reset decoder state (e.g. after a transport reconnect).
    pub fn reset(&mut self) {
        self.state = DecoderState::ReadingHeader { collected: 0 };
    }
}

// ── Encoder ──────────────────────────────────────────────────

/// Encode a FlatBuffer payload into a v2 length-prefixed frame.
///
/// Writes `[LE-u32 total_len][flags][payload]` into `out_buf`.
/// `total_len` = `1 (flags) + payload.len()`.
/// Returns the total number of bytes written.
pub fn encode_frame(payload: &[u8], out_buf: &mut [u8]) -> Option<usize> {
    encode_frame_with_flags(payload, 0, out_buf)
}

/// Encode a payload with explicit flags.
pub fn encode_frame_with_flags(payload: &[u8], flags: u8, out_buf: &mut [u8]) -> Option<usize> {
    let total_content_len = 1 + payload.len();
    let wire_len = V1_HEADER_SIZE + total_content_len;

    if wire_len > out_buf.len() || payload.len() > MAX_FRAME_SIZE {
        return None;
    }

    let len_bytes = (total_content_len as u32).to_le_bytes();
    out_buf[..4].copy_from_slice(&len_bytes);
    out_buf[4] = flags;
    out_buf[5..5 + payload.len()].copy_from_slice(payload);

    Some(wire_len)
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_no_flags() {
        let payload = b"hello world";
        let mut buf = [0u8; 64];
        let n = encode_frame(payload, &mut buf).unwrap();

        let mut dec = FrameDecoder::new();
        let result = dec.feed(&buf[..n]).unwrap();
        assert_eq!(result, payload);
        assert_eq!(dec.last_header().flags, 0);
    }

    #[test]
    fn roundtrip_with_flags() {
        let payload = b"compressed data";
        let mut buf = [0u8; 64];
        let n = encode_frame_with_flags(payload, FLAG_COMPRESSED, &mut buf).unwrap();

        let mut dec = FrameDecoder::new();
        let result = dec.feed(&buf[..n]).unwrap();
        assert_eq!(result, payload);
        assert!(dec.last_header().is_compressed());
        assert!(!dec.last_header().is_chunked());
    }

    #[test]
    fn chunked_flags() {
        let mut buf = [0u8; 64];
        let flags = FLAG_CHUNKED | FLAG_LAST_CHUNK;
        let n = encode_frame_with_flags(b"end", flags, &mut buf).unwrap();

        let mut dec = FrameDecoder::new();
        let _ = dec.feed(&buf[..n]).unwrap();
        let hdr = dec.last_header();
        assert!(hdr.is_chunked());
        assert!(hdr.is_last_chunk());
        assert!(!hdr.is_compressed());
    }

    #[test]
    fn partial_feed() {
        let payload = b"test payload";
        let mut buf = [0u8; 64];
        let n = encode_frame(payload, &mut buf).unwrap();

        let mut dec = FrameDecoder::new();
        assert!(dec.feed(&buf[..3]).is_none());
        assert!(dec.feed(&buf[3..8]).is_none());
        let result = dec.feed(&buf[8..n]).unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn rejects_oversized_frame() {
        let mut buf = [0u8; 8];
        buf[..4].copy_from_slice(&(MAX_FRAME_SIZE as u32 + 100).to_le_bytes());
        buf[4] = 0;

        let mut dec = FrameDecoder::new();
        assert!(dec.feed(&buf).is_none());
    }

    #[test]
    fn rejects_invalid_flags() {
        let mut buf = [0u8; 16];
        let payload = b"hi";
        let total = 1 + payload.len();
        buf[..4].copy_from_slice(&(total as u32).to_le_bytes());
        buf[4] = 0x80;
        buf[5..5 + payload.len()].copy_from_slice(payload);

        let mut dec = FrameDecoder::new();
        assert!(dec.feed(&buf[..5 + payload.len()]).is_none());
    }

    #[test]
    fn encode_too_large_returns_none() {
        let big = [0u8; MAX_FRAME_SIZE + 1];
        let mut buf = [0u8; MAX_FRAME_SIZE + 10];
        assert!(encode_frame(&big, &mut buf).is_none());
    }

    #[test]
    fn reset_clears_state() {
        let mut dec = FrameDecoder::new();
        let _ = dec.feed(&[0x05, 0x00, 0x00]);
        dec.reset();

        let payload = b"after reset";
        let mut buf = [0u8; 64];
        let n = encode_frame(payload, &mut buf).unwrap();
        let result = dec.feed(&buf[..n]).unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn multiple_frames_sequential() {
        let p1 = b"frame one";
        let p2 = b"frame two";
        let mut buf = [0u8; 128];
        let n1 = encode_frame(p1, &mut buf).unwrap();
        let n2 = encode_frame(p2, &mut buf[n1..]).unwrap();

        let mut dec = FrameDecoder::new();
        let r1 = dec.feed(&buf[..n1]).unwrap().to_vec();
        assert_eq!(r1.as_slice(), p1);

        let r2 = dec.feed(&buf[n1..n1 + n2]).unwrap().to_vec();
        assert_eq!(r2.as_slice(), p2);
    }
}
