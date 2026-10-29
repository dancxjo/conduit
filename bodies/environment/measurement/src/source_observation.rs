//! Sensor-independent environmental Source Info and its exact finite Form.
//! Units, field meaning and representation live in this checked contract once;
//! each value carries only decimal coordinates, support/status and timing.
//! A value confers no membership, Play authority, or authenticated MCU Boot.
extern crate alloc;
use crate::{EnvironmentalReading, ReadingField};
use alloc::{vec, vec::Vec};
use conduit_core::{
    self as c, validate_canonical_structured_value as validate,
    PreparedStructuredComposer as Composer, StructuredInfoRefusal as R, StructuredInfoType as Type,
};
pub const INFO: &str = "environment/source-observation@1";
pub const FORM: &str = "environment/source-observation-decimal-coordinates@1";
pub const FORM_BYTES: usize = 82;
pub const MAXIMUM_BYTES: usize = 128;
pub const SOURCE_KIND: &str = "environment/read-environmental-source@1";
pub const PUBLICATION_KIND: &str = "environment/publish-environmental-observation@1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFailure {
    ProviderFailed,
    CaptureTimeout,
    InvalidSample,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSample {
    pub reading: EnvironmentalReading,
    /// Reported correlation within the selected source session, not a global ID.
    pub source_counter: u64,
    /// Unauthenticated source coordinate; never labelled as this Host's Boot time.
    pub reported_source_ms: Option<u64>,
}
// Exact typed view is transient; retained owners store the 82-byte Form.
// Boxing here would allocate on every read and erase Copy value semantics.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceOutcome {
    Available(SourceSample),
    Unavailable(SourceFailure),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceObservation {
    /// Actual producing Host monotonic time supplied by its admitted adapter.
    pub observed_at_host_boot_ms: u64,
    pub outcome: SourceOutcome,
}
pub struct ObservationContract {
    value_type: Type,
    schema: Vec<u8>,
}
impl ObservationContract {
    pub fn prepare() -> Result<Self, R> {
        let value_type = Type::leaf(INFO.into())?;
        let schema = value_type.canonical_bytes()?;
        Ok(Self { value_type, schema })
    }
    pub fn value_type(&self) -> &Type {
        &self.value_type
    }
    pub fn decode(&self, bytes: &[u8]) -> Result<SourceObservation, R> {
        if bytes.len() > MAXIMUM_BYTES {
            return Err(R::CanonicalEncodingTooLarge);
        }
        let value = validate(bytes)?;
        if value.type_bytes() != self.schema {
            return Err(R::WrongType);
        }
        decode_form(value.primitive_bytes(INFO)?)
    }
    /// Exact Fore for the selected ordinary HostCallBack or publication sink.
    /// Preparing this contract installs no native provider or execution authority.
    pub fn kind(&self, publication: bool) -> Result<c::Kind, R> {
        let port = c::PortDescriptor {
            port_id: if publication { "value" } else { "observation" }.into(),
            value_kind: self.value_type.profile()?.value_kind().clone(),
            direction: if publication {
                c::PortDirection::Input
            } else {
                c::PortDirection::Output
            },
            temporal: c::PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        };
        Ok(c::Kind {
            kind_id: if publication {
                PUBLICATION_KIND
            } else {
                SOURCE_KIND
            }
            .into(),
            kind_contract_revision: FORM.into(),
            startup_parameters: vec![],
            shorthand: None,
            inputs: if publication {
                vec![port.clone()]
            } else {
                vec![c::PortDescriptor {
                    port_id: "request".into(),
                    value_kind: Type::leaf(c::EMPTY_INFO_ID.into())?
                        .profile()?
                        .value_kind()
                        .clone(),
                    direction: c::PortDirection::Input,
                    temporal: c::PortTemporal::Flow { closes: true },
                    abnormal_kind: None,
                }]
            },
            outputs: if publication { vec![] } else { vec![port] },
            configuration: vec![],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_BYTES as u32,
            },
        })
    }
}
/// The Form uses fixed reviewed units: Celsius, percent, Pascal. Decimal
/// coefficients/exponents stay exact; conversion refuses inexact values rather
/// than rounding. Unsupported fields have zero coordinate backing. Source clock
/// metadata remains explicitly untrusted and optional. Legacy DHT Forms differ.
pub fn encode_form(value: SourceObservation) -> Result<[u8; FORM_BYTES], R> {
    let mut bytes = [0; FORM_BYTES];
    bytes[0] = 1;
    bytes[2..10].copy_from_slice(&value.observed_at_host_boot_ms.to_le_bytes());
    match value.outcome {
        SourceOutcome::Available(sample) => {
            if sample.source_counter == 0 {
                return Err(R::MalformedCanonicalEncoding);
            }
            bytes[10..18].copy_from_slice(&sample.source_counter.to_le_bytes());
            if let Some(ms) = sample.reported_source_ms {
                bytes[18] = 1;
                bytes[19..27].copy_from_slice(&ms.to_le_bytes());
            }
            encode_quantity(
                sample.reading.temperature(),
                c::Unit::Celsius,
                &mut bytes[28..46],
            )?;
            for (field, unit, flag, offset) in [
                (sample.reading.relative_humidity(), c::Unit::Percent, 1, 46),
                (
                    sample.reading.atmospheric_pressure(),
                    c::Unit::Pascal,
                    2,
                    64,
                ),
            ] {
                if let ReadingField::Available(q) = field {
                    bytes[27] |= flag;
                    encode_quantity(q, unit, &mut bytes[offset..offset + 18])?;
                }
            }
        }
        SourceOutcome::Unavailable(reason) => {
            bytes[1] = match reason {
                SourceFailure::ProviderFailed => 1,
                SourceFailure::CaptureTimeout => 2,
                SourceFailure::InvalidSample => 3,
            }
        }
    }
    Ok(bytes)
}
fn encode_quantity(q: c::Quantity, unit: c::Unit, out: &mut [u8]) -> Result<(), R> {
    let q = q
        .convert_to_decimal(unit)
        .map_err(|_| R::MalformedCanonicalEncoding)?;
    out[..16].copy_from_slice(&q.coefficient().to_le_bytes());
    out[16..18].copy_from_slice(&q.exponent().to_le_bytes());
    Ok(())
}
fn decode_quantity(bytes: &[u8], unit: c::Unit) -> Result<c::Quantity, R> {
    let coefficient = i128::from_le_bytes(
        bytes[..16]
            .try_into()
            .map_err(|_| R::MalformedCanonicalEncoding)?,
    );
    let exponent = i16::from_le_bytes(
        bytes[16..18]
            .try_into()
            .map_err(|_| R::MalformedCanonicalEncoding)?,
    );
    let q = c::Quantity::from_decimal(coefficient, exponent, unit)
        .map_err(|_| R::MalformedCanonicalEncoding)?;
    // Require the canonical decimal representation, not equivalent encodings.
    if q.coefficient() != coefficient || q.exponent() != exponent {
        return Err(R::MalformedCanonicalEncoding);
    }
    Ok(q)
}
pub fn decode_form(bytes: &[u8]) -> Result<SourceObservation, R> {
    if bytes.len() != FORM_BYTES || bytes[0] != 1 {
        return Err(R::MalformedCanonicalEncoding);
    }
    let observed_at_host_boot_ms = u64::from_le_bytes(bytes[2..10].try_into().unwrap());
    let outcome = match bytes[1] {
        0 => {
            if bytes[27] & !3 != 0 || bytes[18] > 1 || (bytes[18] == 0 && bytes[19..27] != [0; 8]) {
                return Err(R::MalformedCanonicalEncoding);
            }
            let source_counter = u64::from_le_bytes(bytes[10..18].try_into().unwrap());
            if source_counter == 0 {
                return Err(R::MalformedCanonicalEncoding);
            }
            let field = |flag, offset, unit| -> Result<ReadingField, R> {
                let b = &bytes[offset..offset + 18];
                if bytes[27] & flag != 0 {
                    Ok(ReadingField::Available(decode_quantity(b, unit)?))
                } else if b == [0; 18] {
                    Ok(ReadingField::Unsupported)
                } else {
                    Err(R::MalformedCanonicalEncoding)
                }
            };
            let reading = EnvironmentalReading::new(
                decode_quantity(&bytes[28..46], c::Unit::Celsius)?,
                field(1, 46, c::Unit::Percent)?,
                field(2, 64, c::Unit::Pascal)?,
            )
            .map_err(|_| R::MalformedCanonicalEncoding)?;
            SourceOutcome::Available(SourceSample {
                reading,
                source_counter,
                reported_source_ms: (bytes[18] == 1)
                    .then(|| u64::from_le_bytes(bytes[19..27].try_into().unwrap())),
            })
        }
        n @ 1..=3 => {
            if bytes[10..].iter().any(|b| *b != 0) {
                return Err(R::MalformedCanonicalEncoding);
            }
            SourceOutcome::Unavailable(match n {
                1 => SourceFailure::ProviderFailed,
                2 => SourceFailure::CaptureTimeout,
                _ => SourceFailure::InvalidSample,
            })
        }
        _ => return Err(R::UnknownVariantTag),
    };
    Ok(SourceObservation {
        observed_at_host_boot_ms,
        outcome,
    })
}
/// One bounded canonical buffer prepared before Play; no per-read schema/unit
/// allocation. Only the selected contract's exact compact Form is accepted.
pub struct PreparedObservationCodec {
    output: Composer,
}
impl PreparedObservationCodec {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        Ok(Self {
            output: Composer::new(contract.value_type(), MAXIMUM_BYTES)?,
        })
    }
    pub fn encode(&mut self, value: SourceObservation) -> Result<&[u8], R> {
        self.output.leaf(&encode_form(value)?)
    }
}

/// Continuity of one actual initialized physical source session. Retain this
/// with the exclusive Base owner across child Plays; never recreate it merely
/// to make a cached sample appear new. It authenticates no MCU Boot identity.
#[derive(Default)]
pub struct SourceContinuity {
    baseline: Option<(u64, Option<u64>, u64)>,
    lost: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceContinuityRefusal {
    NoNewObservation,
    SessionLost,
    HostClockReversed,
    InvalidCounter,
}
impl SourceContinuity {
    /// First observation establishes a baseline but is not a fresh acquisition.
    /// Only a later counter advance can satisfy a bounded acquisition request.
    /// A provider must wait within its admitted deadline, then report timeout
    /// or no new observation, without resealing stale data with a new timestamp.
    pub fn observe(
        &mut self,
        sample: &SourceSample,
        host_observed_ms: u64,
    ) -> Result<(), SourceContinuityRefusal> {
        use SourceContinuityRefusal as E;
        if self.lost {
            return Err(E::SessionLost);
        }
        if sample.source_counter == 0 {
            return Err(E::InvalidCounter);
        }
        let Some((counter, reported, host_ms)) = self.baseline else {
            self.baseline = Some((
                sample.source_counter,
                sample.reported_source_ms,
                host_observed_ms,
            ));
            return Err(E::NoNewObservation);
        };
        if host_observed_ms < host_ms {
            return Err(E::HostClockReversed);
        }
        if sample.source_counter < counter
            || reported.is_some() != sample.reported_source_ms.is_some()
            || matches!((reported,sample.reported_source_ms),(Some(prior),Some(next)) if next<prior)
        {
            self.lost = true;
            return Err(E::SessionLost);
        }
        if sample.source_counter == counter {
            self.baseline = Some((counter, reported, host_observed_ms));
            return Err(E::NoNewObservation);
        }
        self.baseline = Some((
            sample.source_counter,
            sample.reported_source_ms,
            host_observed_ms,
        ));
        Ok(())
    }
}
