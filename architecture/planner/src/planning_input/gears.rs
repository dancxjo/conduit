//! Reusable, retiring, or individually decoded checked Gear material.
#[cfg(feature = "indexed-source")]
use super::connection_gear::ConnectionFacts;
use super::connection_gear::ConnectionGear;
use crate::PlannerError;
use alloc::{borrow::Cow, vec::Vec};
use conduit_core::{ConfigurationEntry, GearId};
use conduit_plot::CheckedGear;

pub(crate) enum PlanningGears<'a> {
    Memory(Cow<'a, [CheckedGear]>),
    #[cfg(feature = "indexed-source")]
    Indexed {
        image: &'a conduit_plot::flat_image::indexed::IndexedSourceImage<'a>,
        connections: Vec<ConnectionFacts>,
    },
}
impl<'a> PlanningGears<'a> {
    pub fn borrowed(gears: &'a [CheckedGear]) -> Self {
        Self::Memory(Cow::Borrowed(gears))
    }
    pub fn owned(gears: Vec<CheckedGear>) -> Self {
        Self::Memory(Cow::Owned(gears))
    }
    #[cfg(feature = "indexed-source")]
    pub fn indexed(image: &'a conduit_plot::flat_image::indexed::IndexedSourceImage<'a>) -> Self {
        Self::Indexed {
            image,
            connections: Vec::with_capacity(image.gear_count()),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Self::Memory(gears) => gears.len(),
            #[cfg(feature = "indexed-source")]
            Self::Indexed { image, .. } => image.gear_count(),
        }
    }
    pub fn get(&self, index: usize) -> Result<Cow<'_, CheckedGear>, PlannerError> {
        match self {
            Self::Memory(gears) => gears.get(index).map(Cow::Borrowed).ok_or_else(|| {
                PlannerError::InvalidPlotIdentity("checked Gear index out of bounds".into())
            }),
            #[cfg(feature = "indexed-source")]
            Self::Indexed { image, .. } => image.gear(index).map(Cow::Owned).map_err(|e| {
                PlannerError::InvalidPlotIdentity(alloc::format!("indexed Source: {e:?}"))
            }),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = Result<Cow<'_, CheckedGear>, PlannerError>> {
        (0..self.len()).map(|i| self.get(i))
    }
    pub fn take_memory_configuration(&mut self, index: usize) -> Vec<ConfigurationEntry> {
        match self {
            Self::Memory(Cow::Borrowed(gears)) => gears[index].configuration.clone(),
            Self::Memory(Cow::Owned(gears)) => core::mem::take(&mut gears[index].configuration),
            #[cfg(feature = "indexed-source")]
            Self::Indexed { .. } => unreachable!("indexed reads return owned Gears"),
        }
    }
    pub fn retain_connection_facts(&mut self, gear: &CheckedGear) {
        #[cfg(not(feature = "indexed-source"))]
        let _ = gear;
        #[cfg(feature = "indexed-source")]
        if let Self::Indexed { connections, .. } = self {
            connections.push(ConnectionFacts {
                gear_id: gear.gear_id.clone(),
                front: gear.checked_front(),
                resource_ports: gear.resource_ports.clone(),
            });
        }
    }
    pub fn connection_gear(&self, id: &GearId) -> Option<ConnectionGear<'_>> {
        match self {
            Self::Memory(gears) => gears
                .iter()
                .find(|g| g.gear_id == *id)
                .map(ConnectionGear::Checked),
            #[cfg(feature = "indexed-source")]
            Self::Indexed { connections, .. } => connections
                .iter()
                .find(|g| g.gear_id == *id)
                .map(ConnectionGear::Prepared),
        }
    }
}
