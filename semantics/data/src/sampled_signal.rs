//! Finite observations of a process on an explicit source clock.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{semantic_digest, Quantity, QuantityUnit, TemporalInstant, TemporalScale};

use crate::{TensorAxisRole, TensorValue};

pub const SAMPLED_SIGNAL_INFO_ID: &str = "data/sampled-signal@1";
pub const MAXIMUM_SIGNAL_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_SIGNAL_PARTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalStart {
    SampleIndex(u64),
    Instant(TemporalInstant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalCadence {
    /// `samples` observations occur during one exact positive time quantity.
    Regular { samples: u64, per: Quantity },
    /// Exact source-clock coordinates. This tensor must be one-dimensional.
    Irregular { coordinates: Box<TensorValue> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalContinuity {
    Continuous,
    Discontinuous { gap_identity: String },
    ClockReset { prior_clock: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampledSignal {
    pub clock_identity: String,
    pub start: SignalStart,
    pub cadence: SignalCadence,
    pub sample_count: u64,
    pub continuity: SignalContinuity,
    /// Shape is `sample × channel...`; backing may be inline or referenced.
    pub samples: TensorValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalWindow {
    pub source_signal: [u8; 32],
    pub source_offset: u64,
    pub sample_count: u64,
    pub start: SignalStart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcatenatedSignal {
    pub clock_identity: String,
    pub start: SignalStart,
    pub cadence: SignalCadence,
    pub sample_count: u64,
    pub element: crate::TensorElement,
    pub sample_shape: Vec<u64>,
    pub axes: Vec<crate::TensorAxis>,
    pub source_parts: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSummary {
    pub clock_identity: String,
    pub start: SignalStart,
    pub sample_count: u64,
    pub continuity: SignalContinuity,
    pub shape: Vec<u64>,
    pub bytes: u64,
    pub content_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampledSignalRefusal {
    InvalidClock,
    InvalidStart,
    InvalidCadence,
    InvalidContinuity,
    EmptySignal,
    TensorInvalid,
    SampleCountMismatch,
    MissingSampleAxis,
    WindowOutOfBounds,
    TemporalOverflow,
    IncompatibleSignals,
    NoncontiguousSignals,
    TooManyParts,
}

impl SampledSignal {
    pub fn validate(&self) -> Result<(), SampledSignalRefusal> {
        identity(&self.clock_identity).map_err(|_| SampledSignalRefusal::InvalidClock)?;
        if self.sample_count == 0 {
            return Err(SampledSignalRefusal::EmptySignal);
        }
        match &self.start {
            SignalStart::SampleIndex(_) => {}
            SignalStart::Instant(instant) => instant
                .validate()
                .map_err(|_| SampledSignalRefusal::InvalidStart)?,
        }
        match &self.cadence {
            SignalCadence::Regular { samples, per } => {
                if *samples == 0
                    || per.value() <= 0
                    || !matches!(
                        per.unit(),
                        QuantityUnit::Second
                            | QuantityUnit::Millisecond
                            | QuantityUnit::Microsecond
                            | QuantityUnit::Nanosecond
                    )
                {
                    return Err(SampledSignalRefusal::InvalidCadence);
                }
            }
            SignalCadence::Irregular { coordinates } => {
                coordinates
                    .validate()
                    .map_err(|_| SampledSignalRefusal::InvalidCadence)?;
                if coordinates.dimensions != [self.sample_count]
                    || coordinates.axes[0].role != TensorAxisRole::Time
                {
                    return Err(SampledSignalRefusal::InvalidCadence);
                }
            }
        }
        match &self.continuity {
            SignalContinuity::Continuous => {}
            SignalContinuity::Discontinuous { gap_identity } => {
                identity(gap_identity).map_err(|_| SampledSignalRefusal::InvalidContinuity)?;
            }
            SignalContinuity::ClockReset { prior_clock } => {
                identity(prior_clock).map_err(|_| SampledSignalRefusal::InvalidContinuity)?;
                if prior_clock == &self.clock_identity {
                    return Err(SampledSignalRefusal::InvalidContinuity);
                }
            }
        }
        self.samples
            .validate()
            .map_err(|_| SampledSignalRefusal::TensorInvalid)?;
        if self.samples.dimensions.first() != Some(&self.sample_count) {
            return Err(SampledSignalRefusal::SampleCountMismatch);
        }
        if self.samples.axes.first().map(|axis| &axis.role) != Some(&TensorAxisRole::Time) {
            return Err(SampledSignalRefusal::MissingSampleAxis);
        }
        Ok(())
    }

    pub fn window(&self, offset: u64, count: u64) -> Result<SignalWindow, SampledSignalRefusal> {
        self.validate()?;
        if count == 0
            || offset
                .checked_add(count)
                .is_none_or(|end| end > self.sample_count)
        {
            return Err(SampledSignalRefusal::WindowOutOfBounds);
        }
        let start = match &self.start {
            SignalStart::SampleIndex(index) => SignalStart::SampleIndex(
                index
                    .checked_add(offset)
                    .ok_or(SampledSignalRefusal::TemporalOverflow)?,
            ),
            SignalStart::Instant(instant) => match &self.cadence {
                SignalCadence::Regular { samples, per }
                    if *samples == 1 && per.unit() == instant.scale.quantity_unit() =>
                {
                    let delta = u64::try_from(per.value())
                        .map_err(|_| SampledSignalRefusal::TemporalOverflow)?
                        .checked_mul(offset)
                        .ok_or(SampledSignalRefusal::TemporalOverflow)?;
                    SignalStart::Instant(TemporalInstant {
                        ticks: instant
                            .ticks
                            .checked_add(delta)
                            .ok_or(SampledSignalRefusal::TemporalOverflow)?,
                        ..instant.clone()
                    })
                }
                _ => return Err(SampledSignalRefusal::InvalidCadence),
            },
        };
        Ok(SignalWindow {
            source_signal: self.semantic_digest()?,
            source_offset: offset,
            sample_count: count,
            start,
        })
    }

    pub fn summary(&self) -> Result<SignalSummary, SampledSignalRefusal> {
        self.validate()?;
        Ok(SignalSummary {
            clock_identity: self.clock_identity.clone(),
            start: self.start.clone(),
            sample_count: self.sample_count,
            continuity: self.continuity.clone(),
            shape: self.samples.dimensions.clone(),
            bytes: self
                .samples
                .byte_count()
                .map_err(|_| SampledSignalRefusal::TensorInvalid)?,
            content_digest: self.samples.content_digest,
        })
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], SampledSignalRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.clock_identity);
        match &self.start {
            SignalStart::SampleIndex(index) => {
                bytes.push(0);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
            SignalStart::Instant(instant) => {
                bytes.push(1);
                bytes.extend_from_slice(&instant.ticks.to_le_bytes());
                bytes.push(scale_tag(instant.scale));
                push_text(&mut bytes, &instant.clock_basis);
                bytes.extend_from_slice(&instant.resolution_ticks.to_le_bytes());
                bytes.extend_from_slice(&instant.uncertainty_ticks.to_le_bytes());
            }
        }
        match &self.cadence {
            SignalCadence::Regular { samples, per } => {
                bytes.push(0);
                bytes.extend_from_slice(&samples.to_le_bytes());
                bytes.extend_from_slice(&per.value().to_le_bytes());
                bytes.push(quantity_unit_tag(per.unit()));
            }
            SignalCadence::Irregular { coordinates } => {
                bytes.push(1);
                bytes.extend_from_slice(
                    &coordinates
                        .semantic_digest()
                        .map_err(|_| SampledSignalRefusal::InvalidCadence)?,
                );
            }
        }
        match &self.continuity {
            SignalContinuity::Continuous => bytes.push(0),
            SignalContinuity::Discontinuous { gap_identity } => {
                bytes.push(1);
                push_text(&mut bytes, gap_identity);
            }
            SignalContinuity::ClockReset { prior_clock } => {
                bytes.push(2);
                push_text(&mut bytes, prior_clock);
            }
        }
        bytes.extend_from_slice(&self.sample_count.to_le_bytes());
        bytes.extend_from_slice(
            &self
                .samples
                .semantic_digest()
                .map_err(|_| SampledSignalRefusal::TensorInvalid)?,
        );
        Ok(semantic_digest(SAMPLED_SIGNAL_INFO_ID, &bytes))
    }
}

pub fn concatenate(parts: &[SampledSignal]) -> Result<ConcatenatedSignal, SampledSignalRefusal> {
    if parts.is_empty() || parts.len() > MAXIMUM_SIGNAL_PARTS {
        return Err(SampledSignalRefusal::TooManyParts);
    }
    for part in parts {
        part.validate()?;
    }
    let first = &parts[0];
    if !matches!(first.start, SignalStart::SampleIndex(_)) {
        return Err(SampledSignalRefusal::IncompatibleSignals);
    }
    let mut next = match first.start {
        SignalStart::SampleIndex(index) => index,
        _ => unreachable!(),
    };
    let mut count = 0_u64;
    let mut digests = Vec::with_capacity(parts.len());
    for part in parts {
        if part.clock_identity != first.clock_identity
            || part.cadence != first.cadence
            || part.samples.element != first.samples.element
            || part.samples.axes != first.samples.axes
            || part.samples.dimensions[1..] != first.samples.dimensions[1..]
            || !matches!(part.continuity, SignalContinuity::Continuous)
        {
            return Err(SampledSignalRefusal::IncompatibleSignals);
        }
        let SignalStart::SampleIndex(start) = part.start else {
            return Err(SampledSignalRefusal::IncompatibleSignals);
        };
        if start != next {
            return Err(SampledSignalRefusal::NoncontiguousSignals);
        }
        next = next
            .checked_add(part.sample_count)
            .ok_or(SampledSignalRefusal::TemporalOverflow)?;
        count = count
            .checked_add(part.sample_count)
            .ok_or(SampledSignalRefusal::TemporalOverflow)?;
        digests.push(part.semantic_digest()?);
    }
    Ok(ConcatenatedSignal {
        clock_identity: first.clock_identity.clone(),
        start: first.start.clone(),
        cadence: first.cadence.clone(),
        sample_count: count,
        element: first.samples.element,
        sample_shape: first.samples.dimensions[1..].to_vec(),
        axes: first.samples.axes.clone(),
        source_parts: digests,
    })
}

fn scale_tag(scale: TemporalScale) -> u8 {
    match scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    }
}

fn quantity_unit_tag(unit: QuantityUnit) -> u8 {
    match unit {
        QuantityUnit::Nanosecond => 0,
        QuantityUnit::Microsecond => 1,
        QuantityUnit::Millisecond => 2,
        QuantityUnit::Second => 3,
        _ => unreachable!("validated cadence only accepts time units"),
    }
}

fn identity(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.len() > MAXIMUM_SIGNAL_IDENTITY_BYTES {
        Err(())
    } else {
        Ok(())
    }
}
fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u16).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}
