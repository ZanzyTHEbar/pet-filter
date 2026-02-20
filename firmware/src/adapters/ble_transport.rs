//! BLE RPC transport — binary RPC over GATT write/notify.
//!
//! Implements a transport layer for the RPC subsystem using BLE GATT
//! characteristics. The client writes RPC frames to the "RPC Write"
//! characteristic, and the device sends responses via GATT notifications
//! on the "RPC Notify" characteristic.
//!
//! ## MTU fragmentation
//!
//! BLE has a limited MTU (typically 23 bytes for BLE 4.x, up to 512
//! for BLE 5.x with DLE). Frames larger than `(mtu - 3)` are split
//! into fragments with a simple sequence header:
//!
//! ```text
//! ┌──────┬──────┬─────────────────────┐
//! │ Seq  │ Flags│  Fragment payload    │
//! │ (1B) │ (1B) │  (mtu - 5 bytes)    │
//! └──────┴──────┴─────────────────────┘
//! ```
//!
//! Flags: bit 0 = "more fragments", bit 1 = "first fragment"
//!
//! ## GATT Service Layout
//!
//! | Characteristic | UUID                                   | Perms       |
//! |---------------|----------------------------------------|-------------|
//! | RPC Write     | `4a650040-b7e4-4b91-a032-5f6c9a1d7e3a` | Write       |
//! | RPC Notify    | `4a650041-b7e4-4b91-a032-5f6c9a1d7e3a` | Notify      |

use log::info;

use crate::rpc::auth::ClientId;
use crate::rpc::transport::Transport;

// ── Constants ────────────────────────────────────────────────

pub const CHAR_RPC_WRITE: u128 = 0x4a650040_b7e4_4b91_a032_5f6c9a1d7e3a;
pub const CHAR_RPC_NOTIFY: u128 = 0x4a650041_b7e4_4b91_a032_5f6c9a1d7e3a;

const DEFAULT_MTU: usize = 23;
const FRAG_HEADER_SIZE: usize = 2;
const MAX_REASSEMBLY_SIZE: usize = 4096;

/// Fragment flag: more fragments follow.
const FRAG_MORE: u8 = 0x01;
/// Fragment flag: this is the first fragment.
const FRAG_FIRST: u8 = 0x02;

// ── Error type ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BleTransportError {
    NotConnected,
    MtuExceeded,
    ReassemblyOverflow,
    SequenceError,
    Io,
}

impl core::fmt::Display for BleTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotConnected => write!(f, "BLE RPC: no client connected"),
            Self::MtuExceeded => write!(f, "BLE RPC: fragment exceeds MTU"),
            Self::ReassemblyOverflow => write!(f, "BLE RPC: reassembly buffer overflow"),
            Self::SequenceError => write!(f, "BLE RPC: fragment sequence error"),
            Self::Io => write!(f, "BLE RPC: GATT I/O error"),
        }
    }
}

// ── Reassembly buffer ────────────────────────────────────────

/// Per-client reassembly buffer for fragmented frames.
struct ReassemblyBuffer {
    data: [u8; MAX_REASSEMBLY_SIZE],
    len: usize,
    expected_seq: u8,
    active: bool,
}

impl ReassemblyBuffer {
    const fn new() -> Self {
        Self {
            data: [0; MAX_REASSEMBLY_SIZE],
            len: 0,
            expected_seq: 0,
            active: false,
        }
    }

    fn reset(&mut self) {
        self.len = 0;
        self.expected_seq = 0;
        self.active = false;
    }

    /// Append a fragment. Returns `Some(&[u8])` when the last fragment
    /// completes the frame.
    fn append(
        &mut self,
        seq: u8,
        flags: u8,
        payload: &[u8],
    ) -> Result<Option<&[u8]>, BleTransportError> {
        let is_first = flags & FRAG_FIRST != 0;
        let has_more = flags & FRAG_MORE != 0;

        if is_first {
            self.reset();
            self.active = true;
            self.expected_seq = 0;
        }

        if !self.active {
            return Err(BleTransportError::SequenceError);
        }

        if seq != self.expected_seq {
            self.reset();
            return Err(BleTransportError::SequenceError);
        }

        if self.len + payload.len() > MAX_REASSEMBLY_SIZE {
            self.reset();
            return Err(BleTransportError::ReassemblyOverflow);
        }

        self.data[self.len..self.len + payload.len()].copy_from_slice(payload);
        self.len += payload.len();
        self.expected_seq = seq.wrapping_add(1);

        if has_more {
            Ok(None)
        } else {
            self.active = false;
            Ok(Some(&self.data[..self.len]))
        }
    }
}

// ── BLE Transport ────────────────────────────────────────────

/// BLE GATT transport with MTU fragmentation.
pub struct BleTransport {
    connected: bool,
    client_id: ClientId,
    mtu: usize,
    reassembly: ReassemblyBuffer,
    read_buf: [u8; MAX_REASSEMBLY_SIZE],
    read_len: usize,
    read_pos: usize,
    write_seq: u8,
}

impl BleTransport {
    pub fn new() -> Self {
        Self {
            connected: false,
            client_id: 0,
            mtu: DEFAULT_MTU,
            reassembly: ReassemblyBuffer::new(),
            read_buf: [0; MAX_REASSEMBLY_SIZE],
            read_len: 0,
            read_pos: 0,
            write_seq: 0,
        }
    }

    pub fn set_mtu(&mut self, mtu: usize) {
        self.mtu = mtu.max(FRAG_HEADER_SIZE + 1);
    }

    pub fn connect(&mut self, client_id: ClientId, mtu: usize) {
        self.connected = true;
        self.client_id = client_id;
        self.mtu = mtu.max(FRAG_HEADER_SIZE + 1);
        self.reassembly.reset();
        self.read_len = 0;
        self.read_pos = 0;
        self.write_seq = 0;
        info!("BLE RPC: client {} connected (MTU={})", client_id, mtu);
    }

    pub fn disconnect(&mut self) {
        info!("BLE RPC: client {} disconnected", self.client_id);
        self.connected = false;
        self.reassembly.reset();
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Handle an incoming GATT write to the RPC Write characteristic.
    ///
    /// Performs reassembly of fragmented frames. When the last fragment
    /// arrives, the complete frame is placed in the read buffer.
    pub fn on_gatt_write(&mut self, data: &[u8]) -> Result<(), BleTransportError> {
        if data.len() < FRAG_HEADER_SIZE {
            return Err(BleTransportError::Io);
        }

        let seq = data[0];
        let flags = data[1];
        let payload = &data[FRAG_HEADER_SIZE..];

        match self.reassembly.append(seq, flags, payload)? {
            Some(complete_frame) => {
                let len = complete_frame.len().min(self.read_buf.len());
                self.read_buf[..len].copy_from_slice(&complete_frame[..len]);
                self.read_len = len;
                self.read_pos = 0;
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Fragment and send a response via GATT notifications.
    ///
    /// The `notify_fn` callback handles the actual BLE notification send.
    pub fn send_fragmented<F>(
        &mut self,
        data: &[u8],
        mut notify_fn: F,
    ) -> Result<(), BleTransportError>
    where
        F: FnMut(&[u8]) -> Result<(), BleTransportError>,
    {
        let max_payload = self.mtu.saturating_sub(3 + FRAG_HEADER_SIZE);
        if max_payload == 0 {
            return Err(BleTransportError::MtuExceeded);
        }

        self.write_seq = 0;
        let chunks: Vec<&[u8]> = data.chunks(max_payload).collect();
        let total = chunks.len();

        for (i, chunk) in chunks.iter().enumerate() {
            let mut frag = [0u8; 512]; // max BLE packet
            frag[0] = self.write_seq;

            let mut flags = 0u8;
            if i == 0 {
                flags |= FRAG_FIRST;
            }
            if i < total - 1 {
                flags |= FRAG_MORE;
            }
            frag[1] = flags;

            let frag_len = FRAG_HEADER_SIZE + chunk.len();
            frag[FRAG_HEADER_SIZE..frag_len].copy_from_slice(chunk);

            notify_fn(&frag[..frag_len])?;
            self.write_seq = self.write_seq.wrapping_add(1);
        }

        Ok(())
    }
}

impl Default for BleTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for BleTransport {
    type Error = BleTransportError;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, BleTransportError> {
        if !self.connected {
            return Err(BleTransportError::NotConnected);
        }
        if self.read_pos >= self.read_len {
            return Ok(0);
        }
        let available = self.read_len - self.read_pos;
        let to_copy = available.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.read_buf[self.read_pos..self.read_pos + to_copy]);
        self.read_pos += to_copy;
        Ok(to_copy)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, BleTransportError> {
        if !self.connected {
            return Err(BleTransportError::NotConnected);
        }
        // BLE writes are handled via send_fragmented + GATT notify
        // The Transport::write path is used for compatibility; actual
        // BLE notification is done through the platform callback.
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), BleTransportError> {
        Ok(())
    }

    fn available(&self) -> bool {
        self.connected && self.read_pos < self.read_len
    }
}

// We need Vec for chunk collection
use std::vec::Vec;

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_fragment_roundtrip() {
        let mut bt = BleTransport::new();
        bt.connect(0, 100);

        let payload = b"hello ble";
        let mut frag = [0u8; 64];
        frag[0] = 0; // seq
        frag[1] = FRAG_FIRST; // first and only (no FRAG_MORE)
        frag[2..2 + payload.len()].copy_from_slice(payload);

        bt.on_gatt_write(&frag[..2 + payload.len()]).unwrap();

        let mut buf = [0u8; 64];
        let n = bt.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], payload);
    }

    #[test]
    fn multi_fragment_reassembly() {
        let mut bt = BleTransport::new();
        bt.connect(0, 100);

        // First fragment
        bt.on_gatt_write(&[0, FRAG_FIRST | FRAG_MORE, b'h', b'e', b'l'])
            .unwrap();

        // no complete frame yet
        let mut buf = [0u8; 32];
        assert_eq!(bt.read(&mut buf).unwrap(), 0);

        // Last fragment
        bt.on_gatt_write(&[1, 0, b'l', b'o']).unwrap();

        let n = bt.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"hello");
    }

    #[test]
    fn sequence_error_resets() {
        let mut bt = BleTransport::new();
        bt.connect(0, 100);

        bt.on_gatt_write(&[0, FRAG_FIRST | FRAG_MORE, b'a'])
            .unwrap();
        // Wrong sequence number (should be 1, not 5)
        assert!(bt.on_gatt_write(&[5, 0, b'b']).is_err());
    }

    #[test]
    fn fragmented_send() {
        let mut bt = BleTransport::new();
        bt.connect(0, 10); // tiny MTU

        let data = b"abcdefghij"; // 10 bytes
        let mut sent = Vec::new();

        bt.send_fragmented(data, |frag| {
            sent.push(frag.to_vec());
            Ok(())
        })
        .unwrap();

        assert!(sent.len() > 1); // should be split
        assert!(sent[0][1] & FRAG_FIRST != 0);
        assert!(sent.last().unwrap()[1] & FRAG_MORE == 0);
    }
}
