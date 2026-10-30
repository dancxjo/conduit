//! Projection from committed typed publications into the recorder's finite Form.
//! This owns no transport, clock, store authority or publication acknowledgement.
use super::{ChunkAppend, PreparedRecordingChunk, RecordingDisposition, RecordingRefusal};
use crate::{
    observation::{ObservationContract, MAXIMUM_BYTES},
    publication::{PublicationChannel as Channel, PublicationContract, PublicationSnapshot},
};
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer,
    PreparedStructuredValueValidator, StructuredInfoRefusal as Refusal,
};

pub struct PreparedRecordingInput {
    validators: [PreparedStructuredValueValidator; 4],
    observation: PreparedStructuredComposer,
}
impl PreparedRecordingInput {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, RecordingRefusal> {
        let validator = |channel: Channel| {
            let contract = PublicationContract::prepare(channel)?;
            PreparedStructuredValueValidator::new(contract.value_type(), channel.maximum_bytes())
        };
        Ok(Self {
            validators: [
                validator(Channel::Reading)?,
                validator(Channel::CaptureTimeout)?,
                validator(Channel::ProviderFailure)?,
                validator(Channel::GeometryRefusal)?,
            ],
            observation: PreparedStructuredComposer::new(
                contract.observation_type(),
                MAXIMUM_BYTES as usize,
            )?,
        })
    }
    /// Root supplies producer time with the event meaning selected for this
    /// chunk. A recorder receipt time must not be substituted for it. The caller
    /// also retains the authenticated producer context selected for the chunk;
    /// this projection cannot establish Host/Boot/Plan/Play from value bytes.
    pub fn append(
        &mut self,
        chunk: &mut PreparedRecordingChunk,
        snapshot: PublicationSnapshot<'_>,
        at_boot_ms: u64,
    ) -> Result<ChunkAppend, RecordingRefusal> {
        self.validators[snapshot.channel.index()].validate(snapshot.canonical)?;
        let disposition = match snapshot.channel {
            Channel::Reading => {
                let result = validate(snapshot.canonical)?;
                if let Some(frame) = result.variant_payload("observation")? {
                    self.observation.record(&[
                        frame
                            .record_field("humidity")?
                            .ok_or(Refusal::WrongRecordFields)?,
                        frame
                            .record_field("temperature")?
                            .ok_or(Refusal::WrongRecordFields)?,
                    ])?;
                    return chunk.append(
                        snapshot.sequence,
                        at_boot_ms,
                        RecordingDisposition::Available,
                        Some(self.observation.encoded()),
                    );
                }
                let failure = result
                    .variant_payload("unavailable")?
                    .ok_or(Refusal::WrongType)?;
                let mut disposition = None;
                for (tag, status) in [
                    ("short-frame", RecordingDisposition::ShortFrame),
                    ("malformed-length", RecordingDisposition::MalformedLength),
                    ("checksum", RecordingDisposition::Checksum),
                    ("invalid-payload", RecordingDisposition::InvalidPayload),
                ] {
                    if failure.variant_payload(tag)?.is_some() {
                        disposition = Some(status);
                        break;
                    }
                }
                disposition.ok_or(Refusal::WrongType)?
            }
            Channel::CaptureTimeout => RecordingDisposition::CaptureTimeout,
            Channel::ProviderFailure => RecordingDisposition::ProviderFailed,
            Channel::GeometryRefusal => RecordingDisposition::GeometryRefused,
        };
        chunk.append(snapshot.sequence, at_boot_ms, disposition, None)
    }
}
