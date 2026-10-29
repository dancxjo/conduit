//! Finite representation of the existing observation/status projection.
//! The clock is explicitly producer-observed Boot milliseconds. This does not
//! claim an acquisition timestamp, provenance, storage or durable acceptance.
use crate::{
    observation::PreparedObservationQuantityForms as Quantities,
    publication::PublicationSnapshot,
    recording::{RecordingDisposition as Disposition, RecordingDispositionForm},
};
use conduit_core::Quantity;
use conduit_environment_face::{SensorFailure as Failure, SensorStatus};
pub const BYTES: usize = 21;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Sequence,
    Channel,
    Type,
    Quantity,
    Disposition,
    Noncanonical,
}
pub struct ObservedRecord {
    pub sequence: u64,
    pub observed_at_boot_ms: u64,
    pub disposition: Disposition,
    pub quantities: Option<(Quantity, Quantity)>,
}
impl ObservedRecord {
    /// The independently bound source supplies the snapshot and commit clock.
    /// Channel validation precedes the shared semantic projection and Form.
    pub fn from_publication(
        snapshot: PublicationSnapshot<'_>,
        observed_at_boot_ms: u64,
    ) -> Result<Self, Refusal> {
        if snapshot.sequence == 0 {
            return Err(Refusal::Sequence);
        }
        snapshot
            .channel
            .validate_value(snapshot.canonical)
            .map_err(|_| Refusal::Type)?;
        let conduit_environment_face::DecodedPublication {
            observation: quantities,
            status,
        } = conduit_environment_face::decode_publication_result(
            snapshot.channel.index(),
            snapshot.canonical,
        )
        .map_err(|_| Refusal::Channel)?;
        let disposition = match status {
            SensorStatus::Available => Disposition::Available,
            SensorStatus::Unavailable(failure) => match failure {
                Failure::CaptureTimeout => Disposition::CaptureTimeout,
                Failure::ProviderFailed => Disposition::ProviderFailed,
                Failure::GeometryRefused => Disposition::GeometryRefused,
                Failure::ShortFrame => Disposition::ShortFrame,
                Failure::MalformedLength => Disposition::MalformedLength,
                Failure::Checksum => Disposition::Checksum,
                Failure::InvalidPayload => Disposition::InvalidPayload,
            },
            SensorStatus::Warming => return Err(Refusal::Disposition),
        };
        Ok(Self {
            sequence: snapshot.sequence,
            observed_at_boot_ms,
            disposition,
            quantities,
        })
    }
    pub fn sensor_status(&self) -> SensorStatus {
        match self.disposition {
            Disposition::Available => SensorStatus::Available,
            Disposition::CaptureTimeout => SensorStatus::Unavailable(Failure::CaptureTimeout),
            Disposition::ProviderFailed => SensorStatus::Unavailable(Failure::ProviderFailed),
            Disposition::GeometryRefused => SensorStatus::Unavailable(Failure::GeometryRefused),
            Disposition::ShortFrame => SensorStatus::Unavailable(Failure::ShortFrame),
            Disposition::MalformedLength => SensorStatus::Unavailable(Failure::MalformedLength),
            Disposition::Checksum => SensorStatus::Unavailable(Failure::Checksum),
            Disposition::InvalidPayload => SensorStatus::Unavailable(Failure::InvalidPayload),
        }
    }
    pub fn encode(&self) -> Result<[u8; BYTES], Refusal> {
        if self.sequence == 0 {
            return Err(Refusal::Sequence);
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.observed_at_boot_ms.to_le_bytes());
        bytes[16] = RecordingDispositionForm::encode(self.disposition)[0];
        match (self.disposition, self.quantities) {
            (Disposition::Available, Some((temperature, humidity))) => bytes[17..].copy_from_slice(
                &Quantities::pack_quantities(temperature, humidity)
                    .map_err(|_| Refusal::Quantity)?,
            ),
            (Disposition::Available, None) => return Err(Refusal::Noncanonical),
            (_, Some(_)) => return Err(Refusal::Noncanonical),
            (_, None) => (),
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() != BYTES {
            return Err(Refusal::Noncanonical);
        }
        let sequence = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        if sequence == 0 {
            return Err(Refusal::Sequence);
        }
        let disposition =
            RecordingDispositionForm::decode(&bytes[16..17]).map_err(|_| Refusal::Disposition)?;
        let quantities = if disposition == Disposition::Available {
            Some(Quantities::unpack_quantities(&bytes[17..]).map_err(|_| Refusal::Quantity)?)
        } else {
            if bytes[17..].iter().any(|b| *b != 0) {
                return Err(Refusal::Noncanonical);
            }
            None
        };
        Ok(Self {
            sequence,
            observed_at_boot_ms: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            disposition,
            quantities,
        })
    }
}

/// Immutable representation identity stored once with a producer scope. It
/// binds the observation Type, existing authored Forms, and observed-clock tag.
/// A basis mismatch prevents decoding old media with changed representations.
pub const DESCRIPTOR_BYTES: usize = 4 + 32 + 3 * 129;
pub fn descriptor(
    contract: &crate::observation::ObservationContract,
) -> Result<[u8; DESCRIPTOR_BYTES], Refusal> {
    let mut bytes = [0; DESCRIPTOR_BYTES];
    bytes[..4].copy_from_slice(b"ORH1"); // version 1: producer-observed Boot ms
    bytes[4..36].copy_from_slice(
        &contract
            .observation_type()
            .semantic_digest()
            .map_err(|_| Refusal::Type)?,
    );
    for (slot, id) in bytes[36..].chunks_exact_mut(129).zip([
        Quantities::TEMPERATURE_FORM_ID,
        Quantities::HUMIDITY_FORM_ID,
        RecordingDispositionForm::IDENTITY,
    ]) {
        if id.len() > 128 {
            return Err(Refusal::Type);
        }
        slot[0] = id.len() as u8;
        slot[1..1 + id.len()].copy_from_slice(id.as_bytes());
    }
    Ok(bytes)
}
