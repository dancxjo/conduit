//! Actual six-file ST7789 Source through the ordinary native Body/kernel path.
//! Scripted native providers prove protocol traffic, not physical panel output.
use super::{frame_chunks::PreparedFrameChunks, minipitft_source};
use crate::{
    digital_base::{DigitalDisposition, DigitalProvider, DigitalRequest},
    display::rgb565::{Orientation, Rgb565Raster},
    monotonic_clock::{codec::ClockDisposition, owner::MonotonicDeadlineProvider},
    protocol_play::NoI2cProvider,
    protocol_source::{PreparedProtocolBodyPlay, PreparedProtocolEntry, prepare_native_peripheral},
    spi_base::{SpiDisposition, SpiProvider, SpiTransaction},
};
use alloc::{rc::Rc, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use core::cell::RefCell;

#[path = "../protocol_source/native_peripheral_preparation/tests/owners.rs"]
mod owners;

// Test expectations only: no protocol actions are generated or sent by Rust.
const INITIAL: [&[u8]; 14] = [
    &[0x01],
    &[0x11],
    &[0x3a],
    &[0x55],
    &[0x36],
    &[0xc0],
    &[0x21],
    &[0x13],
    &[0x29],
    &[0x2a],
    &[0, 0, 0, 0xef],
    &[0x2b],
    &[0, 0x50, 1, 0x3f],
    &[0x2c],
];
const DC: [bool; 15] = [
    false, false, false, true, false, true, false, false, false, false, true, false, true, false,
    true,
];
struct Trace {
    spi: usize,
    frames: usize,
    failed_at: Option<usize>,
    command_data: bool,
    dc: Vec<bool>,
    backlight: Vec<bool>,
    delays: Vec<u64>,
    now: u64,
    frame: Vec<u8>,
    revoked: [bool; 4],
}
impl Trace {
    fn new(frame: Vec<u8>, failed_at: Option<usize>) -> Self {
        Self {
            spi: 0,
            frames: 0,
            failed_at,
            command_data: false,
            dc: Vec::with_capacity(15),
            backlight: Vec::with_capacity(3),
            delays: Vec::with_capacity(5),
            now: 0,
            frame,
            revoked: [false; 4],
        }
    }
}
struct Spi(Rc<RefCell<Trace>>);
impl SpiProvider for Spi {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        let mut trace = self.0.borrow_mut();
        assert!(!trace.revoked[0]);
        let index = trace.spi;
        trace.spi += 1;
        if index < INITIAL.len() {
            assert_eq!(request.output(), INITIAL[index]);
            assert_eq!(trace.command_data, DC[index]);
        } else {
            let offset = trace.frames * 512;
            assert_eq!(request.output().len(), 512);
            assert!(trace.command_data);
            assert_eq!(request.output(), &trace.frame[offset..offset + 512]);
        }
        if trace.failed_at == Some(index) {
            return Err(SpiDisposition::TimedOut);
        }
        if index >= INITIAL.len() {
            trace.frames += 1;
        }
        input.fill(0);
        Ok(())
    }
    fn revoke(&mut self) {
        self.0.borrow_mut().revoked[0] = true;
    }
}
struct Contact {
    trace: Rc<RefCell<Trace>>,
    index: usize,
}
impl DigitalProvider for Contact {
    fn access(&mut self, request: DigitalRequest) -> Result<bool, DigitalDisposition> {
        let mut trace = self.trace.borrow_mut();
        assert!(!trace.revoked[self.index]);
        let DigitalRequest::Drive(high) = request else {
            panic!("protocol must drive exact contacts")
        };
        if self.index == 1 {
            if high {
                assert_eq!(
                    trace.frames, 225,
                    "backlight before final frame acknowledgement"
                );
                assert_eq!(trace.spi, INITIAL.len() + 225);
            }
            trace.backlight.push(high);
        } else {
            trace.command_data = high;
            trace.dc.push(high);
        }
        Ok(high)
    }
    fn revoke(&mut self) {
        let mut trace = self.trace.borrow_mut();
        if trace.revoked[self.index] {
            return;
        }
        trace.revoked[self.index] = true;
        if self.index == 1 {
            trace.backlight.push(false);
        } else {
            trace.command_data = false;
        }
    }
}
struct Clock(Rc<RefCell<Trace>>);
impl MonotonicDeadlineProvider for Clock {
    fn poll_until(&mut self, deadline: u64) -> Result<Option<u64>, ClockDisposition> {
        let mut trace = self.0.borrow_mut();
        assert!(!trace.revoked[3]);
        if deadline != 0 {
            let delta = deadline
                .checked_sub(trace.now)
                .expect("monotonic Source deadlines");
            assert!(delta > 0);
            trace.delays.push(delta);
            trace.now = deadline;
        }
        Ok(Some(trace.now))
    }
    fn revoke(&mut self) {
        self.0.borrow_mut().revoked[3] = true;
    }
}
struct Output {
    port: PortDescriptor,
    value: ValuePayload,
}
fn output(entry: &PreparedProtocolEntry, name: &str) -> Output {
    let port = entry
        .expanded()
        .front
        .outputs()
        .iter()
        .find(|port| port.port_id.as_str() == name)
        .unwrap()
        .clone();
    Output {
        value: ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        },
        port,
    }
}
struct Fixture {
    play: PreparedProtocolBodyPlay<NoI2cProvider, Clock>,
    trace: Rc<RefCell<Trace>>,
    input: PortDescriptor,
    input_value: ValuePayload,
    chunks: PreparedFrameChunks,
    requested: Output,
    completed: Output,
    refused: Output,
    pixels: Vec<u32>,
}
fn fixture(failed_at: Option<usize>) -> Fixture {
    let entry = minipitft_source::entry().unwrap();
    let input = entry.expanded().front.inputs()[0].clone();
    assert_eq!(input.port_id.as_str(), "chunks");
    let schema = entry.input_schema(&input.port_id).unwrap();
    let chunks = PreparedFrameChunks::new(&schema, 240, 240, 256, Orientation::Rotate180).unwrap();
    assert_eq!(chunks.count(), 225);
    let pixels: Vec<_> = (0..240 * 240u32)
        .map(|i| ((i & 255) << 16) | (((i / 240) & 255) << 8) | ((i / 256) & 255))
        .collect();
    let raster = Rgb565Raster::new(&pixels, 240, 240).unwrap();
    let mut frame = vec![0; 240 * 240 * 2];
    raster
        .encode_into(0, 240 * 240, Orientation::Rotate180, &mut frame)
        .unwrap();
    let trace = Rc::new(RefCell::new(Trace::new(frame, failed_at)));
    let gear = |name| {
        entry
            .expanded()
            .expanded
            .gears
            .iter()
            .find(|gear| {
                gear.kind_id.as_str() == "machine/digital/access"
                    && gear.gear_id.as_str().rsplit('/').next() == Some(name)
            })
            .unwrap()
            .gear_id
            .clone()
    };
    let (session, host, mut owners) = owners::prepare(
        &entry,
        [gear("backlight"), gear("command-data")],
        Spi(trace.clone()),
        [
            Contact {
                trace: trace.clone(),
                index: 1,
            },
            Contact {
                trace: trace.clone(),
                index: 2,
            },
        ],
        Clock(trace.clone()),
    );
    owners.sign_storage = minipitft_source::sign_storage(chunks.count(), 4).unwrap();
    let requested = output(&entry, "requested");
    let completed = output(&entry, "completed");
    let refused = output(&entry, "refused");
    let play = prepare_native_peripheral(entry, session, host, owners, 0)
        .unwrap_or_else(|failure| panic!("native preparation: {:?}", failure.reason));
    assert_eq!(trace.borrow().spi, 0);
    assert!(trace.borrow().backlight.is_empty());
    assert!(trace.borrow().dc.is_empty());
    assert!(trace.borrow().delays.is_empty());
    let input_value = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    Fixture {
        play,
        trace,
        input,
        input_value,
        chunks,
        requested,
        completed,
        refused,
        pixels,
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Completed(u64),
    Refused(u64),
    Exhausted,
}
fn drive(f: &mut Fixture) -> Outcome {
    let raster = Rgb565Raster::new(&f.pixels, 240, 240).unwrap();
    let mut next = 0;
    for _ in 0..2_000_000 {
        f.play.step().unwrap_or_else(|error| {
            panic!(
                "kernel step after {} SPI and {} frame writes, next chunk {next}: {error:?}",
                f.trace.borrow().spi,
                f.trace.borrow().frames
            )
        });
        if let Some(sequence) = f
            .play
            .output_into(&f.requested.port.port_id, &mut f.requested.value)
            .unwrap()
        {
            let value = validate_canonical_structured_value(&f.requested.value.encoded).unwrap();
            let bytes = value
                .record_field("index")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap();
            let index = u64::from_le_bytes(bytes.try_into().unwrap());
            assert_eq!(index, next);
            let encoded = f.chunks.encode(&raster, index).unwrap();
            f.input_value.encoded.clear();
            f.input_value.encoded.extend_from_slice(encoded);
            if matches!(
                f.play
                    .admit_input(&f.input.port_id, next, &f.input_value)
                    .unwrap(),
                RemoteIngressOutcome::Accepted { .. }
            ) {
                f.play
                    .complete_output(&f.requested.port.port_id, sequence)
                    .unwrap();
                next += 1;
                if next == f.chunks.count() {
                    f.play.close_input(&f.input.port_id).unwrap();
                }
            }
        }
        if let Some(sequence) = f
            .play
            .output_into(&f.refused.port.port_id, &mut f.refused.value)
            .unwrap()
        {
            let reason = u64::from_le_bytes(f.refused.value.encoded.as_slice().try_into().unwrap());
            f.play
                .complete_output(&f.refused.port.port_id, sequence)
                .unwrap();
            return Outcome::Refused(reason);
        }
        if let Some(sequence) = f
            .play
            .output_into(&f.completed.port.port_id, &mut f.completed.value)
            .unwrap()
        {
            let count =
                u64::from_le_bytes(f.completed.value.encoded.as_slice().try_into().unwrap());
            assert_eq!(next, count);
            f.play
                .complete_output(&f.completed.port.port_id, sequence)
                .unwrap();
            return Outcome::Completed(count);
        }
    }
    Outcome::Exhausted
}
#[test]
fn actual_st7789_source_streams_all_225_typed_chunks_before_illumination_without_growth() {
    let mut f = fixture(None);
    f.play.start().unwrap();
    let mut outcome = Outcome::Exhausted;
    let allocations = crate::test_allocations::allocations(|| {
        outcome = drive(&mut f);
    });
    assert_eq!(outcome, Outcome::Completed(225));
    assert_eq!(
        allocations, 0,
        "stepping and typed frame codec must not grow"
    );
    assert_eq!(f.trace.borrow().dc, DC);
    assert_eq!(f.trace.borrow().delays, [150, 500, 10, 10, 500]);
    assert_eq!(f.trace.borrow().frames, 225);
    assert_eq!(f.trace.borrow().backlight, [false, true]);
    f.play.cancel().unwrap();
    assert_eq!(f.trace.borrow().revoked, [true; 4]);
    assert_eq!(f.trace.borrow().backlight.last(), Some(&false));
    assert!(!f.trace.borrow().command_data);
}
#[test]
fn actual_st7789_spi_failure_reports_source_refusal_and_requests_safe_low() {
    let mut f = fixture(Some(0));
    f.play.start().unwrap();
    assert_eq!(drive(&mut f), Outcome::Refused(30));
    assert_eq!(f.trace.borrow().backlight, [false, false]);
    assert_eq!(f.trace.borrow().spi, 1);
    assert_eq!(f.trace.borrow().frames, 0);
    f.play.cancel().unwrap();
    assert_eq!(f.trace.borrow().revoked, [true; 4]);
    assert_eq!(f.trace.borrow().backlight.last(), Some(&false));
}

#[test]
fn actual_st7789_mid_frame_transfer_loss_never_completes_or_illuminates() {
    let delivered = 112;
    let mut f = fixture(Some(INITIAL.len() + delivered));
    f.play.start().unwrap();
    assert_eq!(drive(&mut f), Outcome::Refused(30));
    assert_eq!(f.trace.borrow().frames, delivered);
    assert_eq!(f.trace.borrow().spi, INITIAL.len() + delivered + 1);
    assert_eq!(f.trace.borrow().backlight, [false, false]);
    assert!(
        f.play
            .output_into(&f.completed.port.port_id, &mut f.completed.value)
            .unwrap()
            .is_none()
    );
    f.play.cancel().unwrap();
    assert_eq!(f.trace.borrow().revoked, [true; 4]);
}

#[test]
fn actual_st7789_pending_chunk_preserves_backpressure_and_cancels_safe() {
    let mut f = fixture(None);
    f.play.start().unwrap();
    let mut request = None;
    for _ in 0..100_000 {
        f.play.step().unwrap();
        request = f
            .play
            .output_into(&f.requested.port.port_id, &mut f.requested.value)
            .unwrap();
        if request.is_some() {
            break;
        }
    }
    assert!(
        request.is_some(),
        "the checked protocol must request its first chunk"
    );
    for _ in 0..1_000 {
        f.play.step().unwrap();
    }
    assert_eq!(f.trace.borrow().spi, INITIAL.len());
    assert_eq!(f.trace.borrow().frames, 0);
    assert_eq!(f.trace.borrow().backlight, [false]);
    assert_eq!(
        f.play
            .output_into(&f.requested.port.port_id, &mut f.requested.value)
            .unwrap(),
        request
    );
    assert!(
        f.play
            .output_into(&f.completed.port.port_id, &mut f.completed.value)
            .unwrap()
            .is_none()
    );
    f.play.cancel().unwrap();
    assert_eq!(f.trace.borrow().revoked, [true; 4]);
    assert_eq!(f.trace.borrow().backlight.last(), Some(&false));
}
