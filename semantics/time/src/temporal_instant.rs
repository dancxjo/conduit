//! Explicit adapters between native portable time meaning and core clock machinery.

use conduit_core::TemporalRelationError;
use conduit_form::rust_binding::NativeBindingRefusal;

use crate::generated::{TemporalInstant, TemporalScale};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalInstantAdapterRefusal {
    NativeBinding(NativeBindingRefusal),
    CoreTemporal(TemporalRelationError),
}

impl From<NativeBindingRefusal> for TemporalInstantAdapterRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::NativeBinding(value)
    }
}

impl From<TemporalRelationError> for TemporalInstantAdapterRefusal {
    fn from(value: TemporalRelationError) -> Self {
        Self::CoreTemporal(value)
    }
}

impl TryFrom<conduit_core::TemporalInstant> for TemporalInstant {
    type Error = TemporalInstantAdapterRefusal;

    fn try_from(value: conduit_core::TemporalInstant) -> Result<Self, Self::Error> {
        value.validate()?;
        Self::new(
            value.clock_basis,
            value.resolution_ticks,
            value.scale.into(),
            value.ticks,
            value.uncertainty_ticks,
        )
        .map_err(Into::into)
    }
}

impl TryFrom<TemporalInstant> for conduit_core::TemporalInstant {
    type Error = TemporalInstantAdapterRefusal;

    fn try_from(value: TemporalInstant) -> Result<Self, Self::Error> {
        let adapted = Self {
            ticks: *value.ticks(),
            scale: (*value.scale()).into(),
            clock_basis: value.clock_basis().clone(),
            resolution_ticks: *value.resolution_ticks(),
            uncertainty_ticks: *value.uncertainty_ticks(),
        };
        adapted.validate()?;
        Ok(adapted)
    }
}

impl From<conduit_core::TemporalScale> for TemporalScale {
    fn from(value: conduit_core::TemporalScale) -> Self {
        match value {
            conduit_core::TemporalScale::Seconds => Self::Seconds,
            conduit_core::TemporalScale::Milliseconds => Self::Milliseconds,
            conduit_core::TemporalScale::Microseconds => Self::Microseconds,
            conduit_core::TemporalScale::Nanoseconds => Self::Nanoseconds,
        }
    }
}

impl From<TemporalScale> for conduit_core::TemporalScale {
    fn from(value: TemporalScale) -> Self {
        match value {
            TemporalScale::Seconds => Self::Seconds,
            TemporalScale::Milliseconds => Self::Milliseconds,
            TemporalScale::Microseconds => Self::Microseconds,
            TemporalScale::Nanoseconds => Self::Nanoseconds,
        }
    }
}
