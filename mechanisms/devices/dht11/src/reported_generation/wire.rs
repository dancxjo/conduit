//! Canonical domain encoding; the embedded original is checked by its own codec.
use super::*;
use alloc::{string::String, vec::Vec};
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredValueValidator,
    StructuredFieldValue, StructuredInfoType as Type, StructuredInfoTypeShape,
    StructuredInfoValue as Value, ValidatedCanonicalStructuredValue as Borrowed,
};

pub(super) fn encode(
    metadata: &Metadata,
    original: &str,
    recovery: ReportedGenerationRecovery,
    predecessor: Option<(u64, u64)>,
) -> Result<Vec<u8>, Refusal> {
    if original.is_empty() || original.len() > MAXIMUM_CHUNK_BYTES {
        return Err(Refusal::Capacity);
    }
    let ty = value_type()?;
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(Refusal::Preparation);
    };
    let mut values = Vec::with_capacity(fields.len());
    for field in fields {
        let name = field.name();
        let text = schema::TEXT_FIELDS
            .iter()
            .position(|candidate| *candidate == name)
            .map(|index| {
                if index < 4 {
                    metadata.producer[index].as_str()
                } else {
                    metadata.channels[(index - 4) / 4][(index - 4) % 4].as_str()
                }
            });
        let bytes = if let Some(text) = text {
            text.as_bytes().to_vec()
        } else if name == "original-recording" {
            original.as_bytes().to_vec()
        } else {
            match name {
                "recovered-chunks" => recovery.chunks.to_le_bytes().to_vec(),
                "recovered-samples" => recovery.samples.to_le_bytes().to_vec(),
                "recovered-first-sequence" => recovery.first_sequence.to_le_bytes().to_vec(),
                "recovered-last-sequence" => recovery.last_sequence.to_le_bytes().to_vec(),
                "samples-outside-view" => recovery.samples_outside_view.to_le_bytes().to_vec(),
                "preceding-history-known" => {
                    alloc::vec![u8::from(recovery.preceding_history_known)]
                }
                "predecessor-known" => alloc::vec![u8::from(predecessor.is_some())],
                "predecessor-sequence" => predecessor.map_or(0, |p| p.0).to_le_bytes().to_vec(),
                "predecessor-boot-ms" => predecessor.map_or(0, |p| p.1).to_le_bytes().to_vec(),
                _ => return Err(Refusal::Preparation),
            }
        };
        values.push(StructuredFieldValue::new(
            name,
            Value::leaf(field.value_type().clone(), bytes)?,
        )?);
    }
    let bytes = Value::record(ty, values)?.canonical_bytes()?;
    if bytes.len() > maximum_canonical_bytes()? as usize {
        return Err(Refusal::Capacity);
    }
    Ok(bytes)
}

pub struct PreparedReportedRecordingGenerationInput {
    value_type: Type,
    validator: PreparedStructuredValueValidator,
    maximum: u32,
}
impl PreparedReportedRecordingGenerationInput {
    pub fn prepare() -> Result<Self, Refusal> {
        let value_type = value_type()?;
        let maximum = maximum_canonical_bytes()?;
        let validator = PreparedStructuredValueValidator::new(&value_type, maximum as usize)?;
        Ok(Self {
            value_type,
            validator,
            maximum,
        })
    }
    pub fn maximum_bytes(&self) -> u32 {
        self.maximum
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.value_type
            .retained_backing_bytes()?
            .checked_add(self.validator.retained_backing_bytes()?)
    }
    /// Hosted preparation/Face projection only. Reported context is validated
    /// against the original codec prefix; it is not authenticated provenance.
    pub fn decode(
        &self,
        canonical: &[u8],
        contract: &ObservationContract,
    ) -> Result<ReportedRecordingGeneration, Refusal> {
        if canonical.len() > self.maximum as usize {
            return Err(Refusal::Capacity);
        }
        self.validator.validate(canonical)?;
        let value = validate(canonical)?;
        let chunks = number(value, "recovered-chunks")?;
        let samples = number(value, "recovered-samples")?;
        if chunks == 0
            || chunks > u64::from(MAXIMUM_GENERATIONS)
            || samples < chunks
            || samples > chunks * MAXIMUM_CHUNK_SAMPLES as u64
            || samples > u64::from(MAXIMUM_RECOVERED_SAMPLES)
        {
            return Err(Refusal::Summary);
        }
        if boolean(value, "preceding-history-known")? {
            return Err(Refusal::Summary);
        }
        let predecessor_known = boolean(value, "predecessor-known")?;
        let predecessor_tuple = (
            number(value, "predecessor-sequence")?,
            number(value, "predecessor-boot-ms")?,
        );
        let predecessor = if predecessor_known {
            Some(predecessor_tuple)
        } else {
            if predecessor_tuple != (0, 0) {
                return Err(Refusal::Summary);
            }
            None
        };
        let first_sequence = number(value, "recovered-first-sequence")?;
        let outside = number(value, "samples-outside-view")?;
        if first_sequence == 0
            || outside > samples
            || (chunks == 1 && predecessor.is_some())
            || (chunks > 1 && predecessor.is_none())
        {
            return Err(Refusal::Summary);
        }
        let original = text(value, "original-recording")?;
        if original.is_empty() || original.len() > MAXIMUM_CHUNK_BYTES {
            return Err(Refusal::Capacity);
        }
        let mut texts = [""; 12];
        for (target, name) in texts.iter_mut().zip(schema::TEXT_FIELDS) {
            *target = text(value, name)?;
        }
        let metadata = Metadata::from_text(
            texts[..4].try_into().map_err(|_| Refusal::Encoding)?,
            [
                texts[4..8].try_into().map_err(|_| Refusal::Encoding)?,
                texts[8..12].try_into().map_err(|_| Refusal::Encoding)?,
            ],
        )?;
        let [host, boot, plan, play] = metadata.producer.clone();
        let (host, boot, plan, play) = (host.into(), boot.into(), plan.into(), play.into());
        let mut reader = PreparedRecordingReader::prepare(
            RecordingContext {
                host: &host,
                boot: &boot,
                plan: &plan,
                play: &play,
            },
            contract,
        )
        .map_err(|_| Refusal::Preparation)?;
        reader.read(original).map_err(Refusal::Content)?;
        let visible = reader.len();
        let first = reader.sample(0).ok_or(Refusal::Empty)?;
        let first_sample = (first.sequence, first.sampled_at_boot_ms);
        let last = reader.sample(visible - 1).ok_or(Refusal::Empty)?.sequence;
        if first_sequence == 0
            || last < first_sequence
            || last - first_sequence < samples - 1
            || number(value, "recovered-last-sequence")? != last
            || samples < visible as u64
            || outside != samples - visible as u64
        {
            return Err(Refusal::Summary);
        }
        if chunks == 1 {
            if predecessor.is_some()
                || samples != visible as u64
                || first_sequence != first_sample.0
            {
                return Err(Refusal::Summary);
            }
        } else {
            let Some((sequence, time)) = predecessor else {
                return Err(Refusal::Summary);
            };
            if sequence == 0
                || sequence >= first_sample.0
                || time > first_sample.1
                || first_sequence > sequence
                || outside < chunks - 1
                || outside > (chunks - 1) * MAXIMUM_CHUNK_SAMPLES as u64
                || sequence - first_sequence < outside - 1
                || (outside == 1 && first_sequence != sequence)
            {
                return Err(Refusal::Summary);
            }
        }
        Ok(ReportedRecordingGeneration {
            metadata,
            original: String::from(original),
            recovery: ReportedGenerationRecovery {
                chunks: chunks as u16,
                samples: samples as u32,
                visible_samples: visible,
                samples_outside_view: outside as u32,
                first_sequence,
                last_sequence: last,
                preceding_history_known: false,
            },
            predecessor,
            reader,
        })
    }
}
fn leaf<'a>(value: Borrowed<'a>, name: &str) -> Result<&'a [u8], Refusal> {
    value
        .record_field(name)?
        .ok_or(Refusal::Encoding)?
        .value_node()
        .get(5..)
        .ok_or(Refusal::Encoding)
}
fn text<'a>(value: Borrowed<'a>, name: &str) -> Result<&'a str, Refusal> {
    core::str::from_utf8(leaf(value, name)?).map_err(|_| Refusal::Encoding)
}
fn number(value: Borrowed<'_>, name: &str) -> Result<u64, Refusal> {
    let bytes = leaf(value, name)?;
    let mut octets = [0; 8];
    if bytes.len() > octets.len() {
        return Err(Refusal::Encoding);
    }
    octets[..bytes.len()].copy_from_slice(bytes);
    Ok(u64::from_le_bytes(octets))
}
fn boolean(value: Borrowed<'_>, name: &str) -> Result<bool, Refusal> {
    match leaf(value, name)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(Refusal::Encoding),
    }
}
