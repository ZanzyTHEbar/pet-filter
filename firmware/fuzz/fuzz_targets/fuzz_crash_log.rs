//! Fuzz target: `CrashLog` ring buffer
//!
//! Exercises the NVS-backed ring buffer by driving arbitrary
//! `write_entry` / `read_all` / `clear` sequences and verifying:
//! - No panics under arbitrary byte inputs
//! - Ring buffer never writes beyond `CRASH_RING_SLOTS` (4) entries
//! - `read_all` always returns at most 4 entries
//!
//! cargo fuzz run fuzz_crash_log

#![no_main]

use libfuzzer_sys::fuzz_target;
use petfilter::diagnostics::{CrashEntry, CrashLog};

// ── In-memory StoragePort for fuzz testing ────────────────────

use petfilter::app::ports::{StorageError, StoragePort};
use std::collections::HashMap;

struct MemStore {
    data: HashMap<String, Vec<u8>>,
}

impl MemStore {
    fn new() -> Self {
        Self { data: HashMap::new() }
    }
}

impl StoragePort for MemStore {
    fn read(&self, ns: &str, key: &str, buf: &mut [u8]) -> Result<usize, StorageError> {
        match self.data.get(&format!("{ns}::{key}")) {
            Some(v) => {
                let n = v.len().min(buf.len());
                buf[..n].copy_from_slice(&v[..n]);
                Ok(n)
            }
            None => Err(StorageError::NotFound),
        }
    }

    fn write(&mut self, ns: &str, key: &str, data: &[u8]) -> Result<(), StorageError> {
        self.data.insert(format!("{ns}::{key}"), data.to_vec());
        Ok(())
    }

    fn exists(&self, ns: &str, key: &str) -> bool {
        self.data.contains_key(&format!("{ns}::{key}"))
    }

    fn delete(&mut self, ns: &str, key: &str) -> Result<(), StorageError> {
        self.data.remove(&format!("{ns}::{key}"));
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut store = MemStore::new();
    let mut log = CrashLog::new();
    log.init(&store);

    // Drive 1–N write operations using fuzz bytes as seeds.
    // Use chunks of 8 bytes: [uptime_u64 = bytes 0..8 implied by len] + [reason idx]
    let num_writes = (data[0] as usize % 8) + 1;
    for i in 0..num_writes {
        let uptime = i as u64 * 1000;
        let reason_len = 1 + (data.get(i + 1).copied().unwrap_or(0) as usize % 30);
        let reason = core::str::from_utf8(&data[1..(1 + reason_len).min(data.len())])
            .unwrap_or("fuzz");
        let pc = u32::from_le_bytes(
            data.get(4..8).and_then(|s| s.try_into().ok()).unwrap_or([0; 4]),
        );

        let entry = CrashEntry::new(uptime, reason, pc);
        log.write_entry(&mut store, &entry);
    }

    // read_all must return at most 4 entries and must not panic.
    let entries = log.read_all(&store);
    assert!(
        entries.len() <= 4,
        "read_all returned {} entries — exceeds ring capacity",
        entries.len()
    );

    // clear must succeed without panics.
    log.clear(&mut store);

    // After clear, read_all must return 0 entries.
    let after_clear = log.read_all(&store);
    assert!(
        after_clear.is_empty(),
        "read_all after clear returned {} entries",
        after_clear.len()
    );
});
