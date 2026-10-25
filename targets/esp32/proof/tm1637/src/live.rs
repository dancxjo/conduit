//! Live Source/Face/Mask bench. It emits no fabricated Body/Plan/Play identity.
//! Native lifecycle admission and AP/HTTP installation remain separate work.
use crate::{capture, electrical::DisplayPins};
use conduit_environment_runtime::{
    display_packets::PreparedDisplayPackets,
    observation::{ObservationContract, PreparedSourceFrameDecoder},
};
use conduit_esp32_pulse_capture::{Esp32PulseCapture, LIMITS};
use conduit_pulse_capture::{CheckedPulseRead, PulseReadDisposition};
use conduit_seven_segment_mask::{DigitWiring, TimedSevenSegmentMask, Tm1637Profile, WireBit};
use conduit_tm1637_wire::{transmit, Packet};
use esp_hal::time::Instant;
pub struct Live {
    decoder: PreparedSourceFrameDecoder,
    packets: PreparedDisplayPackets,
    request: CheckedPulseRead,
    profile: Tm1637Profile,
    mask: TimedSevenSegmentMask,
    mask_started_ms: u64,
}
impl Live {
    pub fn prepare() -> Self {
        let contract = ObservationContract::prepare().unwrap();
        let request = PreparedSourceFrameDecoder::capture_request(LIMITS).unwrap();
        let decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
        let packets = PreparedDisplayPackets::prepare().unwrap();
        conduit_environment_http::initialize(conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(PreparedSourceFrameDecoder::SOURCE_DOCUMENT_ID.into()),
            checked_plot_id: Some(PreparedSourceFrameDecoder::CHECKED_PLOT_ID.into()),
            expanded_plot_id: Some(PreparedSourceFrameDecoder::EXPANDED_PLOT_ID.into()),
            plan_id: None,
            active_play_id: None,
            sign_ids: alloc::vec![],
        });
        let profile = Tm1637Profile {
            digits: core::array::from_fn(|i| DigitWiring {
                address: i as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: [None; 4],
            colon: Some(WireBit { address: 1, bit: 7 }),
        };
        profile.validate().unwrap();
        esp_println::println!("{{\"event\":\"live-source-prepared\",\"body_installed\":false,\"heap_used\":{},\"heap_free\":{}}}",esp_alloc::HEAP.used(),esp_alloc::HEAP.free());
        Self {
            decoder,
            packets,
            request,
            profile,
            mask: TimedSevenSegmentMask::prepare(
                conduit_seven_segment_mask::SevenSegmentLook::fahrenheit().with_colon_fraction(),
                3_000,
                24 * 60 * 60 * 1_000,
            )
            .unwrap(),
            mask_started_ms: Instant::now().duration_since_epoch().as_millis(),
        }
    }
    pub fn sample(&mut self, sensor: &mut Esp32PulseCapture<'_>) {
        let now = Instant::now().duration_since_epoch().as_millis();
        match capture::read(sensor, self.request) {
            Ok(capture) if capture.disposition == PulseReadDisposition::Complete => {
                esp_println::println!("capture: {:?}", capture.wire);
                let mut wire = [0; 5];
                wire.copy_from_slice(&capture.wire[..5]);
                let result = self.decoder.decode(capture.length, wire).unwrap();
                conduit_environment_http::publish(0, result, now);
            }
            Ok(capture) => conduit_environment_http::publish(
                if capture.disposition == PulseReadDisposition::TimedOut {
                    1
                } else {
                    2
                },
                &[],
                now,
            ),
            Err(_) => conduit_environment_http::publish(3, &[], now),
        }
    }
    pub fn display(&mut self, pins: &mut DisplayPins<'_>) {
        let now = Instant::now().duration_since_epoch().as_millis();
        let snapshot = conduit_environment_http::seven_segment_snapshot_with_mask(
            now,
            now.saturating_sub(self.mask_started_ms),
            &self.profile,
            &self.mask,
        )
        .unwrap();
        let registers = self.profile.map(&snapshot.show).unwrap();
        let encoded = self.packets.encode(registers, 3, true).unwrap();
        let packets = encoded.packets().map(|bytes| Packet { bytes });
        let result = transmit(pins, &packets, || false);
        let reading = conduit_environment_http::response(now, true);
        let body = reading.split_once("\r\n\r\n").unwrap().1;
        esp_println::println!("{{\"event\":\"live-source-reading\",\"reading\":{},\"display\":\"{:?}\",\"registers\":{:?},\"heap_used\":{}}}",body,result,registers,esp_alloc::HEAP.used());
    }
}
extern crate alloc;
