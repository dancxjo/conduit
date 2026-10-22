//! Observe the Source result after Play, preserving semantic quantity Forms.
extern crate alloc;
use crate::usb_link::UsbLinkSession;
use conduit_core::{validate_canonical_structured_value as validate, Quantity};
use core::fmt::Write;
// Keep this post-Play future off the earlier kernel preparation stack/frame.
// The caller enters a separate finite preparation phase after dropping Play.
#[inline(never)]
pub fn prepare<'a>(
    link: &'a mut UsbLinkSession,
    index: usize,
    bytes: &'a [u8],
    basis: &'a conduit_face::FaceBasis,
    fresh: bool,
) -> core::pin::Pin<alloc::boxed::Box<impl core::future::Future<Output = ()> + 'a>> {
    alloc::boxed::Box::pin(report(link, index, bytes, basis, fresh))
}
async fn report(
    link: &mut UsbLinkSession,
    index: usize,
    bytes: &[u8],
    basis: &conduit_face::FaceBasis,
    fresh: bool,
) {
    use conduit_environment_face::{SensorFailure as Failure, SensorStatus as Status};
    let mut observation = None;
    let status;
    let mut line = heapless::String::<768>::new();
    if index == 0 {
        let result = validate(bytes).unwrap();
        if let Some(frame) = result.variant_payload("observation").unwrap() {
            let quantity = |name| {
                let leaf = frame.record_field(name).unwrap().unwrap();
                Quantity::decode(&leaf.value_node()[5..]).unwrap()
            };
            let t = quantity("temperature");
            let h = quantity("humidity");
            observation = Some((t, h));
            status = Status::Available;
            writeln!(line,"{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"observation\",\"temperature\":{{\"type\":\"Temperature\",\"coefficient\":{},\"exponent\":{},\"unit\":\"°C\"}},\"humidity\":{{\"type\":\"Ratio\",\"coefficient\":{},\"exponent\":{},\"unit\":\"%\"}}}}",t.coefficient(),t.exponent(),h.coefficient(),h.exponent()).unwrap();
        } else {
            let failure = result.variant_payload("unavailable").unwrap().unwrap();
            let reason = [
                "short-frame",
                "malformed-length",
                "checksum",
                "invalid-payload",
            ]
            .into_iter()
            .find(|tag| failure.variant_payload(tag).unwrap().is_some())
            .unwrap();
            status = Status::Unavailable(match reason {
                "short-frame" => Failure::ShortFrame,
                "malformed-length" => Failure::MalformedLength,
                "checksum" => Failure::Checksum,
                _ => Failure::InvalidPayload,
            });
            writeln!(line,"{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"unavailable\",\"reason\":\"{reason}\"}}").unwrap();
        }
    } else {
        let reason = [
            "decoded",
            "timed-out",
            "provider-failed",
            "geometry-refused",
        ][index];
        status = Status::Unavailable(match index {
            1 => Failure::CaptureTimeout,
            2 => Failure::ProviderFailed,
            _ => Failure::GeometryRefused,
        });
        writeln!(line,"{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"unavailable\",\"reason\":\"{reason}\"}}").unwrap();
    }
    let _ = link.write_text(&line).await;
    crate::kernel_manifest::report(link, observation, status, basis, fresh).await;
}
