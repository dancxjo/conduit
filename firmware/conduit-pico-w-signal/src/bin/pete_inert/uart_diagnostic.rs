//! Bounded, read-only observations of the sole Create UART owner.

use embassy_rp::uart::Error;
use portable_atomic::{AtomicU32, AtomicU8, Ordering};

pub const MAX_FRAME_BYTES: usize = 30;

pub struct Snapshot {
    pub window_start_ms: u32,
    pub rx_bytes: u32,
    pub tx_bytes: u32,
    pub valid_frames: u32,
    pub corrupt_frames: u32,
    pub resync_discarded_bytes: u32,
    pub timeouts: u32,
    pub overruns: u32,
    pub breaks: u32,
    pub parity_errors: u32,
    pub framing_errors: u32,
    pub other_errors: u32,
    pub first_byte_ms: Option<u32>,
    pub last_corrupt_packet_id: u8,
    pub last_corrupt_frame_len: usize,
    pub last_corrupt_frame: [u8; MAX_FRAME_BYTES],
}

static WINDOW_START_MS: AtomicU32 = AtomicU32::new(0);
static RX_BYTES: AtomicU32 = AtomicU32::new(0);
static TX_BYTES: AtomicU32 = AtomicU32::new(0);
static VALID_FRAMES: AtomicU32 = AtomicU32::new(0);
static CORRUPT_FRAMES: AtomicU32 = AtomicU32::new(0);
static RESYNC_DISCARDED_BYTES: AtomicU32 = AtomicU32::new(0);
static TIMEOUTS: AtomicU32 = AtomicU32::new(0);
static OVERRUNS: AtomicU32 = AtomicU32::new(0);
static BREAKS: AtomicU32 = AtomicU32::new(0);
static PARITY_ERRORS: AtomicU32 = AtomicU32::new(0);
static FRAMING_ERRORS: AtomicU32 = AtomicU32::new(0);
static OTHER_ERRORS: AtomicU32 = AtomicU32::new(0);
static FIRST_BYTE_MS: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_CORRUPT_GENERATION: AtomicU32 = AtomicU32::new(0);
static LAST_CORRUPT_PACKET_ID: AtomicU8 = AtomicU8::new(0);
static LAST_CORRUPT_FRAME_LEN: AtomicU8 = AtomicU8::new(0);
static LAST_CORRUPT_FRAME: [AtomicU8; MAX_FRAME_BYTES] =
    [const { AtomicU8::new(0) }; MAX_FRAME_BYTES];

pub fn start(now_ms: u32) {
    WINDOW_START_MS.store(now_ms, Ordering::Release);
}

pub fn record_tx(bytes: usize) {
    TX_BYTES.fetch_add(bytes as u32, Ordering::Relaxed);
}

pub fn record_rx(now_ms: u32) {
    RX_BYTES.fetch_add(1, Ordering::Relaxed);
    let _ = FIRST_BYTE_MS.compare_exchange(u32::MAX, now_ms, Ordering::AcqRel, Ordering::Acquire);
}

pub fn record_discard() {
    RESYNC_DISCARDED_BYTES.fetch_add(1, Ordering::Relaxed);
}

pub fn record_timeout() {
    TIMEOUTS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_error(error: Error) {
    let counter = match error {
        Error::Overrun => &OVERRUNS,
        Error::Break => &BREAKS,
        Error::Parity => &PARITY_ERRORS,
        Error::Framing => &FRAMING_ERRORS,
        _ => &OTHER_ERRORS,
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

pub fn record_frame(packet_id: u8, frame: &[u8], valid: bool) {
    if valid {
        VALID_FRAMES.fetch_add(1, Ordering::Relaxed);
        return;
    }
    CORRUPT_FRAMES.fetch_add(1, Ordering::Relaxed);
    LAST_CORRUPT_GENERATION.fetch_add(1, Ordering::AcqRel);
    LAST_CORRUPT_PACKET_ID.store(packet_id, Ordering::Relaxed);
    for (index, slot) in LAST_CORRUPT_FRAME.iter().enumerate() {
        slot.store(frame.get(index).copied().unwrap_or(0), Ordering::Relaxed);
    }
    LAST_CORRUPT_FRAME_LEN.store(frame.len().min(MAX_FRAME_BYTES) as u8, Ordering::Relaxed);
    LAST_CORRUPT_GENERATION.fetch_add(1, Ordering::Release);
}

pub fn snapshot() -> Snapshot {
    let (packet_id, frame_len, frame) = loop {
        let before = LAST_CORRUPT_GENERATION.load(Ordering::Acquire);
        if before & 1 != 0 {
            continue;
        }
        let packet_id = LAST_CORRUPT_PACKET_ID.load(Ordering::Acquire);
        let frame_len = usize::from(LAST_CORRUPT_FRAME_LEN.load(Ordering::Acquire));
        let mut frame = [0; MAX_FRAME_BYTES];
        for (destination, source) in frame.iter_mut().zip(LAST_CORRUPT_FRAME.iter()) {
            *destination = source.load(Ordering::Acquire);
        }
        if before == LAST_CORRUPT_GENERATION.load(Ordering::Acquire) {
            break (packet_id, frame_len, frame);
        }
    };
    let first_byte_ms = FIRST_BYTE_MS.load(Ordering::Acquire);
    Snapshot {
        window_start_ms: WINDOW_START_MS.load(Ordering::Acquire),
        rx_bytes: RX_BYTES.load(Ordering::Acquire),
        tx_bytes: TX_BYTES.load(Ordering::Acquire),
        valid_frames: VALID_FRAMES.load(Ordering::Acquire),
        corrupt_frames: CORRUPT_FRAMES.load(Ordering::Acquire),
        resync_discarded_bytes: RESYNC_DISCARDED_BYTES.load(Ordering::Acquire),
        timeouts: TIMEOUTS.load(Ordering::Acquire),
        overruns: OVERRUNS.load(Ordering::Acquire),
        breaks: BREAKS.load(Ordering::Acquire),
        parity_errors: PARITY_ERRORS.load(Ordering::Acquire),
        framing_errors: FRAMING_ERRORS.load(Ordering::Acquire),
        other_errors: OTHER_ERRORS.load(Ordering::Acquire),
        first_byte_ms: (first_byte_ms != u32::MAX).then_some(first_byte_ms),
        last_corrupt_packet_id: packet_id,
        last_corrupt_frame_len: frame_len,
        last_corrupt_frame: frame,
    }
}
