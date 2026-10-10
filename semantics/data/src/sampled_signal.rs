//! Finite observations of a process on an explicit source clock.

use alloc::vec::Vec;
use conduit_core::{semantic_digest, QuantityDimension, TemporalScale};
use conduit_plot::rust_binding::BoundedSequence;

use crate::{
    ConcatenatedSignal, SampledSignal, SampledSignalRefusal, SignalCadence, SignalContinuity,
    SignalIdentity, SignalStart, SignalSummary, SignalWindow, TensorAxisRole,
};

pub const SAMPLED_SIGNAL_INFO_ID: &str = "data/sampled-signal@1";
pub const MAXIMUM_SIGNAL_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_SIGNAL_PARTS: usize = 64;

impl SignalStart {
    pub fn at_sample(index: u64) -> Self {
        Self::sample_index(index).expect("every U64 sample index is valid")
    }

    pub fn at_instant(
        instant: conduit_core::TemporalInstant,
    ) -> Result<Self, SampledSignalRefusal> {
        let instant = instant
            .try_into()
            .map_err(|_| SampledSignalRefusal::InvalidStart)?;
        Self::instant(instant).map_err(|_| SampledSignalRefusal::InvalidStart)
    }
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
                .value()
                .validate()
                .map_err(|_| SampledSignalRefusal::InvalidStart)?,
        }
        match &self.cadence {
            SignalCadence::Regular(regular) => {
                if *regular.samples() == 0
                    || regular.per().coefficient() <= 0
                    || regular.per().dimension() != QuantityDimension::Time
                {
                    return Err(SampledSignalRefusal::InvalidCadence);
                }
            }
            SignalCadence::Irregular(irregular) => {
                let coordinates = irregular.coordinates();
                coordinates
                    .validate()
                    .map_err(|_| SampledSignalRefusal::InvalidCadence)?;
                if coordinates.dimensions.as_slice() != [self.sample_count]
                    || coordinates.axes[0].role != TensorAxisRole::Time
                {
                    return Err(SampledSignalRefusal::InvalidCadence);
                }
            }
        }
        match &self.continuity {
            SignalContinuity::Continuous => {}
            SignalContinuity::Discontinuous(_) => {}
            SignalContinuity::ClockReset(reset) => {
                if reset.prior_clock() == &self.clock_identity {
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
            SignalStart::SampleIndex(index) => SignalStart::at_sample(
                index
                    .index()
                    .checked_add(offset)
                    .ok_or(SampledSignalRefusal::TemporalOverflow)?,
            ),
            SignalStart::Instant(instant) => match &self.cadence {
                SignalCadence::Regular(regular)
                    if *regular.samples() == 1
                        && regular.per().unit()
                            == conduit_core::TemporalScale::from(*instant.value().scale())
                                .quantity_unit() =>
                {
                    let delta = regular
                        .per()
                        .convert_to_u64(regular.per().unit())
                        .map_err(|_| SampledSignalRefusal::TemporalOverflow)?
                        .checked_mul(offset)
                        .ok_or(SampledSignalRefusal::TemporalOverflow)?;
                    let core: conduit_core::TemporalInstant = instant
                        .value()
                        .clone()
                        .try_into()
                        .map_err(|_| SampledSignalRefusal::InvalidStart)?;
                    SignalStart::at_instant(conduit_core::TemporalInstant {
                        ticks: core
                            .ticks
                            .checked_add(delta)
                            .ok_or(SampledSignalRefusal::TemporalOverflow)?,
                        ..core
                    })?
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
            shape: BoundedSequence::try_from_iter(self.samples.dimensions.iter().copied())
                .map_err(|_| SampledSignalRefusal::TensorInvalid)?,
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
                bytes.extend_from_slice(&index.index().to_le_bytes());
            }
            SignalStart::Instant(instant) => {
                let instant: conduit_core::TemporalInstant = instant
                    .value()
                    .clone()
                    .try_into()
                    .map_err(|_| SampledSignalRefusal::InvalidStart)?;
                bytes.push(1);
                bytes.extend_from_slice(&instant.ticks.to_le_bytes());
                bytes.push(scale_tag(instant.scale));
                push_text(&mut bytes, &instant.clock_basis);
                bytes.extend_from_slice(&instant.resolution_ticks.to_le_bytes());
                bytes.extend_from_slice(&instant.uncertainty_ticks.to_le_bytes());
            }
        }
        match &self.cadence {
            SignalCadence::Regular(regular) => {
                bytes.push(0);
                bytes.extend_from_slice(&regular.samples().to_le_bytes());
                bytes.extend_from_slice(&regular.per().encode());
            }
            SignalCadence::Irregular(irregular) => {
                bytes.push(1);
                bytes.extend_from_slice(
                    &irregular
                        .coordinates()
                        .semantic_digest()
                        .map_err(|_| SampledSignalRefusal::InvalidCadence)?,
                );
            }
        }
        match &self.continuity {
            SignalContinuity::Continuous => bytes.push(0),
            SignalContinuity::Discontinuous(gap) => {
                bytes.push(1);
                push_text(&mut bytes, gap.gap_identity());
            }
            SignalContinuity::ClockReset(reset) => {
                bytes.push(2);
                push_text(&mut bytes, reset.prior_clock());
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
    let mut next = match &first.start {
        SignalStart::SampleIndex(index) => *index.index(),
        _ => unreachable!(),
    };
    let mut count = 0_u64;
    let mut digests = Vec::with_capacity(parts.len());
    for part in parts {
        if part.clock_identity != first.clock_identity
            || part.cadence != first.cadence
            || part.samples.element != first.samples.element
            || part.samples.axes != first.samples.axes
            || part.samples.dimensions.as_slice()[1..] != first.samples.dimensions.as_slice()[1..]
            || !matches!(part.continuity, SignalContinuity::Continuous)
        {
            return Err(SampledSignalRefusal::IncompatibleSignals);
        }
        let SignalStart::SampleIndex(start) = &part.start else {
            return Err(SampledSignalRefusal::IncompatibleSignals);
        };
        if *start.index() != next {
            return Err(SampledSignalRefusal::NoncontiguousSignals);
        }
        next = next
            .checked_add(part.sample_count)
            .ok_or(SampledSignalRefusal::TemporalOverflow)?;
        count = count
            .checked_add(part.sample_count)
            .ok_or(SampledSignalRefusal::TemporalOverflow)?;
        digests.push(
            SignalIdentity::new(part.semantic_digest()?)
                .expect("a semantic digest is exactly 32 bytes"),
        );
    }
    Ok(ConcatenatedSignal {
        clock_identity: first.clock_identity.clone(),
        start: first.start.clone(),
        cadence: first.cadence.clone(),
        sample_count: count,
        element: first.samples.element,
        sample_shape: BoundedSequence::try_from_iter(
            first.samples.dimensions.as_slice()[1..].iter().copied(),
        )
        .expect("a tensor has at most seven non-sample dimensions"),
        axes: first.samples.axes.clone(),
        source_parts: BoundedSequence::try_from_iter(digests)
            .expect("concatenation admits at most 64 source parts"),
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
