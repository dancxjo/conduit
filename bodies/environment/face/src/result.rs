//! Publication of the existing committed observation and three typed capture
//! outcomes. Canonical sensor decoding remains owned by the authored Plot.
use crate::{PreparedObservationFace, PublicationRefusal, SensorFailure, SensorStatus};
use conduit_core::{validate_canonical_structured_value as validate, Quantity};
impl PreparedObservationFace {
    pub fn publish_result(
        &mut self,
        index: usize,
        bytes: &[u8],
        now: u64,
    ) -> Result<(), PublicationRefusal> {
        let (observation, sensor_status) = decode_observation_result(index, bytes)?;
        match self.publish(observation, sensor_status, now) {
            Err(PublicationRefusal::InvalidTemperature | PublicationRefusal::InvalidHumidity) => {
                self.publish(
                    None,
                    SensorStatus::Unavailable(SensorFailure::InvalidPayload),
                    now,
                )
            }
            result => result,
        }
    }
}

/// Decode the existing semantic publication channels. This does not authenticate
/// a producer or validate a selected channel contract; Hosts retain those duties.
pub fn decode_observation_result(
    index: usize,
    bytes: &[u8],
) -> Result<(Option<(Quantity, Quantity)>, SensorStatus), PublicationRefusal> {
    let mut observation = None;
    let mut status = match index {
        0 => "invalid-payload",
        1 => "timed-out",
        2 => "provider-failed",
        3 => "geometry-refused",
        _ => return Err(PublicationRefusal::InvalidStatus),
    };
    if index == 0 {
        if let Ok(result) = validate(bytes) {
            if let Ok(Some(frame)) = result.variant_payload("observation") {
                let quantity = |name| {
                    let leaf = frame.record_field(name).ok()??;
                    let kind = if name == "temperature" {
                        conduit_core::TEMPERATURE_INFO_ID
                    } else {
                        conduit_core::RATIO_INFO_ID
                    };
                    Quantity::decode(leaf.primitive_bytes(kind).ok()?).ok()
                };
                observation = quantity("temperature").zip(quantity("humidity"));
                if observation.is_some() {
                    status = "available";
                }
            } else if let Ok(Some(failure)) = result.variant_payload("unavailable") {
                for reason in [
                    "short-frame",
                    "malformed-length",
                    "checksum",
                    "invalid-payload",
                ] {
                    if matches!(failure.variant_payload(reason), Ok(Some(_))) {
                        status = reason;
                        break;
                    }
                }
            }
        }
    }
    let sensor_status = match status {
        "available" => SensorStatus::Available,
        "timed-out" => SensorStatus::Unavailable(SensorFailure::CaptureTimeout),
        "provider-failed" => SensorStatus::Unavailable(SensorFailure::ProviderFailed),
        "geometry-refused" => SensorStatus::Unavailable(SensorFailure::GeometryRefused),
        "short-frame" => SensorStatus::Unavailable(SensorFailure::ShortFrame),
        "malformed-length" => SensorStatus::Unavailable(SensorFailure::MalformedLength),
        "checksum" => SensorStatus::Unavailable(SensorFailure::Checksum),
        _ => SensorStatus::Unavailable(SensorFailure::InvalidPayload),
    };
    Ok((observation, sensor_status))
}
