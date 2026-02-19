//! Transport abstraction — any byte-oriented channel.
//!
//! Concrete implementations:
//! - UART serial (USB CDC on ESP32-S3)
//! - BLE GATT characteristic (write + notify)
//! - TCP socket (over WiFi)
//!
//! The RPC engine is generic over `Transport`, so adding a new
//! transport requires zero changes to the RPC logic.

/// Byte-oriented transport channel.
pub trait Transport {
    /// Error type for this transport.
    type Error: core::fmt::Debug;

    /// Read up to `buf.len()` bytes into `buf`.
    /// Returns the number of bytes actually read.
    /// Returns 0 if no data is available (non-blocking).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;

    /// Write `data` to the transport.
    /// Returns the number of bytes actually written.
    fn write(&mut self, data: &[u8]) -> Result<usize, Self::Error>;

    /// Flush any buffered output.
    fn flush(&mut self) -> Result<(), Self::Error>;

    /// Check if data is available for reading.
    fn available(&self) -> bool;
}

/// A null transport that discards all writes and never reads.
/// Useful as a default when no RPC client is connected.
pub struct NullTransport;

impl Transport for NullTransport {
    type Error = ();

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, ()> {
        Ok(0)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, ()> {
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), ()> {
        Ok(())
    }

    fn available(&self) -> bool {
        false
    }
}
