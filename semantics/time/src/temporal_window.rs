//! Exact finite windows over comparable temporal instants.

use serde::{Deserialize, Serialize};

use crate::{TemporalInstant, TemporalRelation, TemporalRelationError};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalBoundary {
    Inclusive,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalWindow {
    start: TemporalInstant,
    start_boundary: TemporalBoundary,
    end: TemporalInstant,
    end_boundary: TemporalBoundary,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalWindowPosition {
    Before,
    Within,
    After,
    Indeterminate,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TemporalWindowRefusal {
    InvalidInstant,
    Incomparable,
    IntervalOverflow,
    Reversed,
    IndeterminateBoundaryOrder,
    Empty,
}

impl TemporalWindow {
    pub fn new(
        start: TemporalInstant,
        start_boundary: TemporalBoundary,
        end: TemporalInstant,
        end_boundary: TemporalBoundary,
    ) -> Result<Self, TemporalWindowRefusal> {
        let value = Self {
            start,
            start_boundary,
            end,
            end_boundary,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), TemporalWindowRefusal> {
        match self
            .start
            .relation_to(&self.end)
            .map_err(map_relation_error)?
        {
            TemporalRelation::Past { .. } => Ok(()),
            TemporalRelation::Present
                if self.start_boundary == TemporalBoundary::Inclusive
                    && self.end_boundary == TemporalBoundary::Inclusive =>
            {
                Ok(())
            }
            TemporalRelation::Present => Err(TemporalWindowRefusal::Empty),
            TemporalRelation::Future { .. } => Err(TemporalWindowRefusal::Reversed),
            TemporalRelation::Indeterminate => {
                Err(TemporalWindowRefusal::IndeterminateBoundaryOrder)
            }
        }
    }

    pub const fn start(&self) -> &TemporalInstant {
        &self.start
    }

    pub const fn start_boundary(&self) -> TemporalBoundary {
        self.start_boundary
    }

    pub const fn end(&self) -> &TemporalInstant {
        &self.end
    }

    pub const fn end_boundary(&self) -> TemporalBoundary {
        self.end_boundary
    }

    pub fn classify(
        &self,
        candidate: &TemporalInstant,
    ) -> Result<TemporalWindowPosition, TemporalWindowRefusal> {
        self.validate()?;
        match candidate
            .relation_to(&self.start)
            .map_err(map_relation_error)?
        {
            TemporalRelation::Past { .. } => return Ok(TemporalWindowPosition::Before),
            TemporalRelation::Present if self.start_boundary == TemporalBoundary::Exclusive => {
                return Ok(TemporalWindowPosition::Before);
            }
            TemporalRelation::Indeterminate => {
                return Ok(TemporalWindowPosition::Indeterminate);
            }
            TemporalRelation::Present | TemporalRelation::Future { .. } => {}
        }
        match candidate
            .relation_to(&self.end)
            .map_err(map_relation_error)?
        {
            TemporalRelation::Past { .. } => Ok(TemporalWindowPosition::Within),
            TemporalRelation::Present if self.end_boundary == TemporalBoundary::Inclusive => {
                Ok(TemporalWindowPosition::Within)
            }
            TemporalRelation::Present | TemporalRelation::Future { .. } => {
                Ok(TemporalWindowPosition::After)
            }
            TemporalRelation::Indeterminate => Ok(TemporalWindowPosition::Indeterminate),
        }
    }
}

fn map_relation_error(error: TemporalRelationError) -> TemporalWindowRefusal {
    match error {
        TemporalRelationError::InvalidInstant => TemporalWindowRefusal::InvalidInstant,
        TemporalRelationError::Incomparable => TemporalWindowRefusal::Incomparable,
        TemporalRelationError::IntervalOverflow => TemporalWindowRefusal::IntervalOverflow,
    }
}
