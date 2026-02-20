//! Async RPC I/O task — reactor-driven multi-client transport bridge.
//!
//! Runs in a dedicated thread using `edge-executor` for cooperative
//! multi-task scheduling and `async-io-mini` for reactor-driven
//! timers (no busy-spinning). Three concurrent futures:
//!
//! 1. **Accept** — polls `try_accept()` every 50ms via reactor timer
//! 2. **Read** — polls `read_client()` every 1ms via reactor timer
//! 3. **Write** — truly async via `RESP_CHANNEL.receive().await`
//!    (wakes instantly when control loop pushes a response)
//!
//! ```text
//!  ┌────────────────────────────────────────────────────────────┐
//!  │  I/O Thread                                                │
//!  │  ┌──────────────────────────────────────────────────────┐  │
//!  │  │  async_io_mini::block_on (drives reactor + futures)  │  │
//!  │  │  ┌──────────────────────────────────────────────────┐│  │
//!  │  │  │  edge_executor::LocalExecutor                    ││  │
//!  │  │  │                                                  ││  │
//!  │  │  │  ┌─────────┐  ┌──────────┐  ┌───────────────┐  ││  │
//!  │  │  │  │ Accept  │  │ Read All │  │ Write (async) │  ││  │
//!  │  │  │  │ 50ms ⏱  │  │ 1ms ⏱   │  │ wake-on-send │  ││  │
//!  │  │  │  └─────────┘  └──────────┘  └───────────────┘  ││  │
//!  │  │  └──────────────────────────────────────────────────┘│  │
//!  │  └──────────────────────────────────────────────────────┘  │
//!  └────────────────────────────────────────────────────────────┘
//! ```

use super::auth::{ClientId, MAX_CLIENTS};
use super::channels::{
    CMD_CHANNEL, CommandMsg, DISCONNECT_CHANNEL, DisconnectMsg, RESP_CHANNEL, ResponseMsg,
};
use super::codec::FrameDecoder;

use crate::events::{push_event, Event};
use core::cell::RefCell;
use core::time::Duration;
use heapless::Vec;
use log::{info, warn};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

const READ_BUF_SIZE: usize = 1024;

/// BLE transport is always assigned slot 0.
pub const BLE_SLOT: ClientId = 0;

/// TCP clients start from slot 1.
pub const TCP_SLOT_START: usize = 1;

const BLE_DEFAULT_MTU: usize = 128;
const BLE_OUTBOX_CAP: usize = 16;

fn ble_transport() -> &'static Mutex<crate::adapters::ble_transport::BleTransport> {
    static BLE_TRANSPORT: OnceLock<Mutex<crate::adapters::ble_transport::BleTransport>> =
        OnceLock::new();
    BLE_TRANSPORT.get_or_init(|| Mutex::new(crate::adapters::ble_transport::BleTransport::new()))
}

fn ble_slot() -> &'static Mutex<IoSlot> {
    static BLE_SLOT_STATE: OnceLock<Mutex<IoSlot>> = OnceLock::new();
    BLE_SLOT_STATE.get_or_init(|| Mutex::new(IoSlot::new()))
}

fn ble_outbox() -> &'static Mutex<VecDeque<Vec<u8, 512>>> {
    static BLE_OUTBOX: OnceLock<Mutex<VecDeque<Vec<u8, 512>>>> = OnceLock::new();
    BLE_OUTBOX.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn queue_ble_response(data: Vec<u8, 512>) {
    let Ok(mut q) = ble_outbox().lock() else {
        warn!("IO[BLE]: outbox lock poisoned");
        return;
    };
    if q.len() >= BLE_OUTBOX_CAP {
        warn!("IO[BLE]: outbox full, dropping response");
        return;
    }
    q.push_back(data);
}

pub fn try_recv_ble_response() -> Option<Vec<u8, 512>> {
    ble_outbox().lock().ok()?.pop_front()
}

pub fn ble_set_connected(mtu: usize) {
    let Ok(mut bt) = ble_transport().lock() else {
        warn!("IO[BLE]: transport lock poisoned");
        return;
    };
    bt.connect(BLE_SLOT, mtu.max(BLE_DEFAULT_MTU));
    if let Ok(mut slot) = ble_slot().lock() {
        slot.reset();
    }
}

pub fn ble_set_disconnected() {
    if let Ok(mut bt) = ble_transport().lock() {
        bt.disconnect();
    }
    if let Ok(mut slot) = ble_slot().lock() {
        slot.reset();
    }
}

// ── Per-client decoder state ─────────────────────────────────

struct IoSlot {
    decoder: FrameDecoder,
}

impl IoSlot {
    fn new() -> Self {
        Self {
            decoder: FrameDecoder::new(),
        }
    }

    fn reset(&mut self) {
        self.decoder.reset();
    }
}

// ── Frame feeding + channel dispatch ─────────────────────────

fn feed_slot_bytes(slot: &mut IoSlot, client_id: ClientId, data: &[u8]) {
    if let Some(frame_bytes) = slot.decoder.feed(data) {
        let mut frame = Vec::new();
        if frame.extend_from_slice(frame_bytes).is_err() {
            warn!("IO[{}]: frame too large for channel buffer", client_id);
            return;
        }
        let msg = CommandMsg { client_id, frame };
        if CMD_CHANNEL.try_send(msg).is_err() {
            warn!("IO[{}]: command channel full, dropping frame", client_id);
        } else {
            // Wake main loop immediately to dispatch inbound RPC command.
            push_event(Event::CommandReceived);
        }
    }
}

fn notify_disconnect(client_id: ClientId) {
    let msg = DisconnectMsg { client_id };
    if DISCONNECT_CHANNEL.try_send(msg).is_err() {
        warn!("IO[{}]: disconnect channel full", client_id);
    }
}

// ── Async I/O loop ───────────────────────────────────────────

type SharedTransport = Rc<RefCell<crate::adapters::tls_transport::TlsTransport>>;
type SharedSlots = Rc<RefCell<[IoSlot; MAX_CLIENTS]>>;

/// Accept task — checks for new TCP connections at 50ms intervals.
/// Lower frequency is fine since connection setup is infrequent.
async fn accept_loop(transport: SharedTransport, slots: SharedSlots) {
    loop {
        {
            let mut t = transport.borrow_mut();
            if let Some(cid) = t.try_accept() {
                info!("IO: client {} connected", cid);
                slots.borrow_mut()[cid as usize].reset();
            }
        }
        async_io_mini::Timer::after(Duration::from_millis(50)).await;
    }
}

/// Read task — polls all connected TCP clients at 1ms intervals.
/// The 1ms reactor timer is wake-based (not thread::sleep), so the
/// executor can service the write task between ticks.
async fn read_loop(transport: SharedTransport, slots: SharedSlots) {
    let mut read_buf = [0u8; READ_BUF_SIZE];
    loop {
        {
            let mut t = transport.borrow_mut();
            let mut s = slots.borrow_mut();
            for cid_idx in TCP_SLOT_START..MAX_CLIENTS {
                let cid = cid_idx as ClientId;
                if !t.is_connected(cid) {
                    continue;
                }
                match t.read_client(cid, &mut read_buf) {
                    Ok(0) => {}
                    Ok(n) => {
                        feed_slot_bytes(&mut s[cid_idx], cid, &read_buf[..n]);
                    }
                    Err(crate::adapters::tls_transport::TlsTransportError::NotConnected) => {
                        info!("IO: client {} disconnected (read)", cid);
                        s[cid_idx].reset();
                        notify_disconnect(cid);
                    }
                    Err(_) => {
                        warn!("IO: client {} read error, disconnecting", cid);
                        t.disconnect(cid);
                        s[cid_idx].reset();
                        notify_disconnect(cid);
                    }
                }
            }
        }
        async_io_mini::Timer::after(Duration::from_millis(1)).await;
    }
}

/// Write task — truly async, wakes instantly when the control loop
/// pushes a response via `RESP_CHANNEL.try_send()`. No polling.
async fn write_loop(transport: SharedTransport, slots: SharedSlots) {
    loop {
        let resp = RESP_CHANNEL.receive().await;
        let cid = resp.client_id;

        let mut t = transport.borrow_mut();
        if !t.is_connected(cid) {
            continue;
        }

        if let Err(e) = t.write_client(cid, &resp.data) {
            warn!("IO: write to client {} failed: {}", cid, e);
            t.disconnect(cid);
            slots.borrow_mut()[cid as usize].reset();
            notify_disconnect(cid);
        } else {
            let _ = t.flush_client(cid);
        }
    }
}

/// Entry point for the I/O thread. Sets up the executor, spawns the
/// three async tasks, and drives them via the `async-io-mini` reactor.
fn run_io_loop(transport: crate::adapters::tls_transport::TlsTransport) {
    let executor: edge_executor::LocalExecutor<'_, 8> = edge_executor::LocalExecutor::new();

    let transport: SharedTransport = Rc::new(RefCell::new(transport));
    let slots: SharedSlots = Rc::new(RefCell::new(core::array::from_fn(|_| IoSlot::new())));

    executor
        .spawn(accept_loop(transport.clone(), slots.clone()))
        .detach();
    executor
        .spawn(read_loop(transport.clone(), slots.clone()))
        .detach();
    executor
        .spawn(write_loop(transport.clone(), slots.clone()))
        .detach();

    info!(
        "IO task started (async, reactor-driven, {} max clients)",
        MAX_CLIENTS
    );

    // async_io_mini::block_on drives the reactor (timers, I/O events)
    // while the executor drives the three spawned tasks.
    futures_lite::future::block_on(executor.run(core::future::pending::<()>()));
}

// ── Thread spawn ─────────────────────────────────────────────

/// Spawn the I/O task in a dedicated thread pinned to Core 0 (PRO_CPU).
///
/// Takes ownership of the TLS transport. The thread runs an async
/// executor with three concurrent tasks: accept, read, and write.
/// Core 0 co-locates with lwIP/BLE for cache-local network I/O.
pub fn spawn(
    transport: crate::adapters::tls_transport::TlsTransport,
) -> std::thread::JoinHandle<()> {
    crate::drivers::task_pin::spawn_on_core(
        crate::drivers::task_pin::Core::Pro,
        12,
        16,
        "rpc-io\0",
        move || run_io_loop(transport),
    )
}

// ── BLE feed (called from BLE GATT handler context) ──────────

/// Feed raw bytes from the BLE transport into slot 0's decoder.
///
/// This is called from the BLE GATT write characteristic handler,
/// which runs in the BT task context — NOT on the I/O thread.
/// Since `CMD_CHANNEL` is `Send`, cross-thread send is safe.
pub fn feed_ble_bytes(data: &[u8]) {
    use crate::rpc::transport::Transport as _;

    let Ok(mut bt) = ble_transport().lock() else {
        warn!("IO[BLE]: transport lock poisoned");
        return;
    };

    if !bt.is_connected() {
        bt.connect(BLE_SLOT, BLE_DEFAULT_MTU);
    }

    if let Err(e) = bt.on_gatt_write(data) {
        warn!("IO[BLE]: invalid fragment: {}", e);
        return;
    }

    let mut buf = [0u8; READ_BUF_SIZE];
    let n = match bt.read(&mut buf) {
        Ok(n) => n,
        Err(e) => {
            warn!("IO[BLE]: read failed: {}", e);
            return;
        }
    };

    if n == 0 {
        return;
    }

    let Ok(mut slot) = ble_slot().lock() else {
        warn!("IO[BLE]: slot lock poisoned");
        return;
    };
    feed_slot_bytes(&mut slot, BLE_SLOT, &buf[..n]);
}

// ── Channel accessors for the control loop ───────────────────

/// Send a response frame to the I/O task for transmission to a client.
///
/// When the control loop calls this, the I/O task's write future
/// wakes instantly via `RESP_CHANNEL.receive().await` — no polling delay.
pub fn send_response(client_id: ClientId, data: Vec<u8, 512>) {
    if client_id == BLE_SLOT {
        queue_ble_response(data);
        return;
    }

    let msg = ResponseMsg { client_id, data };
    if RESP_CHANNEL.try_send(msg).is_err() {
        warn!("RPC: response channel full for client {}", client_id);
    }
}

/// Try to receive an inbound command from the I/O task.
pub fn try_recv_command() -> Option<CommandMsg> {
    CMD_CHANNEL.try_receive().ok()
}

/// Try to receive a disconnect notification.
pub fn try_recv_disconnect() -> Option<DisconnectMsg> {
    DISCONNECT_CHANNEL.try_receive().ok()
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ble_slot_constant() {
        assert_eq!(BLE_SLOT, 0);
        assert_eq!(TCP_SLOT_START, 1);
    }

    #[test]
    fn feed_slot_bytes_no_panic_on_partial() {
        let mut slot = IoSlot::new();
        feed_slot_bytes(&mut slot, 1, &[0x04, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn ble_response_uses_ble_outbox() {
        let mut data = Vec::<u8, 512>::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03]).unwrap();
        send_response(BLE_SLOT, data);
        let popped = try_recv_ble_response().expect("ble outbox empty");
        assert_eq!(&popped[..], &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn feed_ble_bytes_accepts_single_fragment_frame() {
        // Ensure no stale commands from prior tests.
        while try_recv_command().is_some() {}

        ble_set_connected(128);

        let payload = [0xAAu8, 0xBB, 0xCC];
        let mut frame = [0u8; 32];
        let n = crate::rpc::codec::encode_frame(&payload, &mut frame).expect("encode frame");
        let frame = &frame[..n];

        // BLE fragment header: [seq, flags], flags=FRAG_FIRST for single fragment.
        let mut frag = [0u8; 64];
        frag[0] = 0;
        frag[1] = 0x02;
        frag[2..2 + frame.len()].copy_from_slice(frame);

        feed_ble_bytes(&frag[..2 + frame.len()]);

        let cmd = try_recv_command().expect("missing BLE command");
        assert_eq!(cmd.client_id, BLE_SLOT);
        assert_eq!(&cmd.frame[..], &payload);

        ble_set_disconnected();
    }
}
