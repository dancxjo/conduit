//! Exact finite windows over comparable temporal instants.

use crate::{
    TemporalBoundary, TemporalInstant, TemporalRelation, TemporalRelationError, TemporalWindow,
    TemporalWindowPosition, TemporalWindowRefusal,
};

impl TemporalWindow {
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
