//! Length-prefix frame codec.
//!
//! Wire format:
//! ```text
//! ┌────────────┬──────────────────────────┐
//! │ Length (4B)│ FlatBuffer payload (N B) │
//! │ LE u32     │                          │
//! └────────────┴──────────────────────────┘
//! ```
//!
//! The codec accumulates incoming bytes into a buffer and yields
//! complete frames. This handles partial reads gracefully — a
//! single `Transport::read` call may return part of the header,
//! part of the payload, or multiple frames concatenated.

/// Maximum frame payload size (protects against memory exhaustion).
const MAX_FRAME_SIZE: usize = 4096;

/// Frame header size (4-byte little-endian length).
const HEADER_SIZE: usize = 4;

/// Decoder state machine.
enum DecoderState {
    /// Waiting for header bytes.
    ReadingHeader { collected: usize },
    /// Header received, reading payload.
    ReadingPayload { expected: usize, collected: usize },
}

/// Streaming frame decoder.
pub struct FrameDecoder {
    state: DecoderState,
    header_buf: [u8; HEADER_SIZE],
    payload_buf: [u8; MAX_FRAME_SIZE],
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            state: DecoderState::ReadingHeader { collected: 0 },
            header_buf: [0; HEADER_SIZE],
            payload_buf: [0; MAX_FRAME_SIZE],
        }
    }

    /// Feed bytes into the decoder.
    ///
    /// Returns `Some(&[u8])` when a complete frame payload is available.
    /// The returned slice is valid until the next call to `feed`.
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
                        let expected = u32::from_le_bytes(self.header_buf) as usize;

                        if expected == 0 || expected > MAX_FRAME_SIZE {
                            // Invalid frame — reset.
                            self.state = DecoderState::ReadingHeader { collected: 0 };
                            continue;
                        }

                        self.state = DecoderState::ReadingPayload {
                            expected,
                            collected: 0,
                        };
                    }
                }

                DecoderState::ReadingPayload { expected, collected } => {
                    let needed = *expected - *collected;
                    let available = data.len() - offset;
                    let to_copy = needed.min(available);

                    self.payload_buf[*collected..*collected + to_copy]
                        .copy_from_slice(&data[offset..offset + to_copy]);

                    *collected += to_copy;
                    offset += to_copy;

                    if *collected == *expected {
                        let len = *expected;
                        self.state = DecoderState::ReadingHeader { collected: 0 };
                        return Some(&self.payload_buf[..len]);
                    }
                }
            }
        }

        None // No complete frame yet.
    }

    /// Reset decoder state (e.g. after a transport reconnect).
    pub fn reset(&mut self) {
        self.state = DecoderState::ReadingHeader { collected: 0 };
    }
}

/// Encode a FlatBuffer payload into a length-prefixed frame.
///
/// Writes `[LE-u32 length][payload]` into `out_buf`.
/// Returns the total number of bytes written.
pub fn encode_frame(payload: &[u8], out_buf: &mut [u8]) -> Option<usize> {
    let total = HEADER_SIZE + payload.len();
    if total > out_buf.len() || payload.len() > MAX_FRAME_SIZE {
        return None;
    }

    let len_bytes = (payload.len() as u32).to_le_bytes();
    out_buf[..HEADER_SIZE].copy_from_slice(&len_bytes);
    out_buf[HEADER_SIZE..total].copy_from_slice(payload);

    Some(total)
}
