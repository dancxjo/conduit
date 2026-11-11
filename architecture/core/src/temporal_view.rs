//! Allocation-free temporal correspondence over owned or canonical instants.
use crate::{
    TemporalRelation, TemporalRelationError, TemporalScale, ValidatedCanonicalStructuredValue,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BorrowedTemporalInstant<'a> {
    pub ticks: u64,
    pub scale: TemporalScale,
    pub clock_basis: &'a str,
    pub resolution_ticks: u64,
    pub uncertainty_ticks: u64,
}
impl<'a> BorrowedTemporalInstant<'a> {
    /// Decode a borrowed instant from an already validated enclosing native Type.
    /// This checks the finite temporal shape; it grants no clock authenticity.
    pub fn from_canonical(
        value: ValidatedCanonicalStructuredValue<'a>,
    ) -> Result<Self, TemporalRelationError> {
        use TemporalRelationError::InvalidInstant as E;
        let field = |name| value.record_field(name).map_err(|_| E)?.ok_or(E);
        let scale = field("scale")?;
        let mut selected = None;
        for (tag, kind) in [
            ("seconds", TemporalScale::Seconds),
            ("milliseconds", TemporalScale::Milliseconds),
            ("microseconds", TemporalScale::Microseconds),
            ("nanoseconds", TemporalScale::Nanoseconds),
        ] {
            if scale.variant_payload(tag).map_err(|_| E)?.is_some() {
                selected = Some(kind);
            }
        }
        let instant = Self {
            ticks: number(field("ticks")?)?,
            scale: selected.ok_or(E)?,
            clock_basis: core::str::from_utf8(leaf(field("clock_basis")?)?).map_err(|_| E)?,
            resolution_ticks: number(field("resolution_ticks")?)?,
            uncertainty_ticks: number(field("uncertainty_ticks")?)?,
        };
        instant.validate()?;
        Ok(instant)
    }
    pub fn validate(&self) -> Result<(), TemporalRelationError> {
        if self.clock_basis.is_empty()
            || self.clock_basis.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || self.resolution_ticks == 0
        {
            Err(TemporalRelationError::InvalidInstant)
        } else {
            Ok(())
        }
    }
    pub fn relation_to(&self, reference: &Self) -> Result<TemporalRelation, TemporalRelationError> {
        self.validate()?;
        reference.validate()?;
        if self.clock_basis != reference.clock_basis || self.scale != reference.scale {
            return Err(TemporalRelationError::Incomparable);
        }
        let source = self.interval()?;
        let target = reference.interval()?;
        if source.1 < target.0 {
            return Ok(TemporalRelation::Past {
                minimum_ticks: target.0 - source.1,
                maximum_ticks: target.1 - source.0,
            });
        }
        if source.0 > target.1 {
            return Ok(TemporalRelation::Future {
                minimum_ticks: source.0 - target.1,
                maximum_ticks: source.1 - target.0,
            });
        }
        if self.ticks == reference.ticks
            && self.uncertainty_ticks == 0
            && reference.uncertainty_ticks == 0
        {
            Ok(TemporalRelation::Present)
        } else {
            Ok(TemporalRelation::Indeterminate)
        }
    }
    pub fn interval(&self) -> Result<(u64, u64), TemporalRelationError> {
        Ok((
            self.ticks
                .checked_sub(self.uncertainty_ticks)
                .ok_or(TemporalRelationError::IntervalOverflow)?,
            self.ticks
                .checked_add(self.uncertainty_ticks)
                .ok_or(TemporalRelationError::IntervalOverflow)?,
        ))
    }
}
fn leaf(value: ValidatedCanonicalStructuredValue<'_>) -> Result<&[u8], TemporalRelationError> {
    let node = value.value_node();
    if node.len() < 5 || node[0] != 0 {
        return Err(TemporalRelationError::InvalidInstant);
    }
    if u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize != node.len() - 5 {
        return Err(TemporalRelationError::InvalidInstant);
    }
    Ok(&node[5..])
}
fn number(value: ValidatedCanonicalStructuredValue<'_>) -> Result<u64, TemporalRelationError> {
    Ok(u64::from_le_bytes(
        leaf(value)?
            .try_into()
            .map_err(|_| TemporalRelationError::InvalidInstant)?,
    ))
}
