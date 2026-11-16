//! Bounded CPU platform observations, enabled only by genuine Started evidence.
use conduit_mcu_host_runtime::execution_workset::StartedEvidence;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    Mutex,
};
static ENABLED: AtomicBool = AtomicBool::new(false);
pub static CAPTURES: AtomicUsize = AtomicUsize::new(0);
pub static WAITS: AtomicUsize = AtomicUsize::new(0);
pub static PROGRAMS: AtomicUsize = AtomicUsize::new(0);
pub static RECEIVES: AtomicUsize = AtomicUsize::new(0);
pub static TRANSPORT_SENDS: AtomicUsize = AtomicUsize::new(0);
static MEDIA: Mutex<Option<Vec<u8>>> = Mutex::new(None);
pub fn open(evidence: &StartedEvidence) {
    assert_ne!(evidence.custody_identity(), &[0; 32]);
    assert!(!ENABLED.swap(true, SeqCst));
}
pub fn enabled() -> bool {
    ENABLED.load(SeqCst)
}
pub fn require() {
    assert!(
        enabled(),
        "provider effect before original authenticated Start"
    );
}
pub fn capture(
    request: conduit_pulse_capture::CheckedPulseRead,
) -> conduit_pulse_capture::PulseReadCapture {
    require();
    let geometry = request.request();
    assert_eq!(geometry.octets, 5);
    assert_eq!(geometry.hold_low_us, crate::config::DHT_START_LOW_US);
    let ordinal = CAPTURES.fetch_add(1, SeqCst);
    let temperature = 23 + (ordinal % 2) as u8;
    conduit_pulse_capture::PulseReadCapture::from_complete_octets(
        conduit_pulse_capture::PulseReadDisposition::Complete,
        &[44, 0, temperature, 0, 44 + temperature],
    )
    .unwrap()
}
pub fn save_media(bytes: Vec<u8>) {
    let mut saved = MEDIA.lock().unwrap();
    assert!(saved.is_none());
    *saved = Some(bytes);
}
pub fn take_media() -> Vec<u8> {
    MEDIA
        .lock()
        .unwrap()
        .take()
        .expect("original quiesced NOR owner")
}
struct Trace {
    clock: bool,
    data: bool,
    active: bool,
    bits: u8,
    byte: u8,
    packet: heapless::Vec<u8, 7>,
    digits: [u8; 4],
    frames: heapless::Vec<[u8; 4], 128>,
    total_frames: usize,
    seen: [bool; 3],
    transactions: usize,
    acks: usize,
}
static WIRE: Mutex<Trace> = Mutex::new(Trace {
    clock: true,
    data: true,
    active: false,
    bits: 0,
    byte: 0,
    packet: heapless::Vec::new(),
    digits: [0; 4],
    frames: heapless::Vec::new(),
    total_frames: 0,
    seen: [false; 3],
    transactions: 0,
    acks: 0,
});
pub fn begin_wire() {
    require();
    WIRE.lock().unwrap().transactions += 1;
}
pub fn clock(released: bool) {
    require();
    let mut t = WIRE.lock().unwrap();
    if !t.clock && released && t.active {
        if t.bits < 8 && t.data {
            t.byte |= 1 << t.bits;
        }
        t.bits = t.bits.saturating_add(1);
    }
    t.clock = released;
}
pub fn data(released: bool) {
    require();
    let mut t = WIRE.lock().unwrap();
    if t.clock && t.data != released {
        if !released {
            t.active = true;
            t.bits = 0;
            t.byte = 0;
            t.packet.clear();
        } else if t.active {
            t.active = false;
            if t.packet.first() == Some(&0xc0) {
                assert_eq!(t.packet.len(), 7);
                let packet = t.packet.clone();
                assert_eq!(&packet[5..], &[0, 0]);
                t.digits.copy_from_slice(&packet[1..5]);
            } else if t.packet.len() == 1 && t.packet[0] & 0xf8 == 0x88 {
                let digits = t.digits;
                t.total_frames += 1;
                for (i, expected) in [
                    [0, 0x07, 0x4f, 0x71],
                    [0, 0x07, 0x6d, 0x71],
                    [0, 0x66, 0x66, 0x76],
                ]
                .iter()
                .enumerate()
                {
                    t.seen[i] |= digits == *expected;
                }
                if t.frames.is_full() {
                    t.frames.remove(0);
                }
                t.frames.push(digits).unwrap();
            } else if !t.packet.is_empty() {
                assert_eq!(t.packet.as_slice(), &[0x40]);
            }
            t.packet.clear();
            t.bits = 0;
            t.byte = 0;
        }
    }
    t.data = released;
}
pub fn ack() -> bool {
    require();
    let mut t = WIRE.lock().unwrap();
    assert_eq!(t.bits, 9);
    let byte = t.byte;
    t.packet.push(byte).unwrap();
    t.bits = 0;
    t.byte = 0;
    t.acks += 1;
    false
}
pub fn frame_count() -> usize {
    WIRE.lock().unwrap().total_frames
}
pub fn check_frames() {
    let t = WIRE.lock().unwrap();
    assert!(t.frames.len() >= 3, "no repeated Source-rendered frames");
    assert!(
        t.frames.iter().any(|f| *f != t.frames[0]),
        "no alternating display pages"
    );
    assert_eq!(t.seen, [true; 3], "73F/75F/44H physical pages absent");
    assert_eq!(t.acks, t.total_frames * 9);
    println!(
        "CPU_WIRE frames={} transactions={} acks={} first={:02x?} last={:02x?}",
        t.total_frames,
        t.transactions,
        t.acks,
        t.frames[0],
        t.frames.last().unwrap()
    );
}
