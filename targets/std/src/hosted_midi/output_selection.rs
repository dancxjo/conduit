use super::{HostedMidiSelection, HostedRawMidiSelection};
use conduit_core::{BootId, HostId, OfferGeneration, RealizationAdvertisement, ResourcePoolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MidiOutputSelection {
    Sequencer(HostedMidiSelection),
    Raw(HostedRawMidiSelection),
}

impl MidiOutputSelection {
    pub(crate) const fn sequencer(selection: HostedMidiSelection) -> Self {
        Self::Sequencer(selection)
    }

    pub(crate) const fn raw(selection: HostedRawMidiSelection) -> Self {
        Self::Raw(selection)
    }

    pub(crate) const fn boot_id(&self) -> &BootId {
        match self {
            Self::Sequencer(selection) => selection.boot_id(),
            Self::Raw(selection) => selection.boot_id(),
        }
    }

    pub(crate) const fn offer_generation(&self) -> OfferGeneration {
        match self {
            Self::Sequencer(selection) => selection.offer_generation(),
            Self::Raw(selection) => selection.offer_generation(),
        }
    }

    pub(crate) fn resource_pool_id(&self) -> ResourcePoolId {
        match self {
            Self::Sequencer(selection) => selection.resource_pool_id(),
            Self::Raw(selection) => selection.resource_pool_id(),
        }
    }

    pub(crate) fn output_realization_advertisement(
        &self,
        host_id: HostId,
    ) -> Result<RealizationAdvertisement, &'static str> {
        match self {
            Self::Sequencer(selection) => selection.output_realization_advertisement(host_id),
            Self::Raw(selection) => selection.output_realization_advertisement(host_id),
        }
    }

    pub(crate) const fn as_sequencer(&self) -> Option<&HostedMidiSelection> {
        match self {
            Self::Sequencer(selection) => Some(selection),
            Self::Raw(_) => None,
        }
    }

    pub(crate) const fn as_raw(&self) -> Option<&HostedRawMidiSelection> {
        match self {
            Self::Raw(selection) => Some(selection),
            Self::Sequencer(_) => None,
        }
    }
}
