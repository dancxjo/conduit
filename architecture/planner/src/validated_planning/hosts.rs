//! Retiring descriptions transfer after ordinary admission. No partial Plan or
//! advertisement escapes this private preparation scope.
use super::*;
pub(crate) enum PlanningHosts<'a> {
    Borrowed(&'a [HostAdvertisement]),
    #[cfg_attr(
        not(feature = "indexed-source"),
        allow(dead_code, reason = "retiring input is used by indexed preparation")
    )]
    Owned(Vec<HostAdvertisement>),
    #[cfg(feature = "indexed-source")]
    Indexed(&'a [crate::indexed_source::IndexedHostOffers<'a>]),
}
pub(super) struct OfferDescription {
    pub inputs: Vec<conduit_core::PortDescriptor>,
    pub outputs: Vec<conduit_core::PortDescriptor>,
    pub semantic_contract: conduit_core::KindSemanticContract,
    pub host_calls: Vec<conduit_core::HostCallRequirement>,
    pub realization_properties: Vec<conduit_core::StructuredConfigurationValue>,
    pub kind_id: conduit_core::KindId,
    pub kind_contract_revision: conduit_core::KindIdentity,
    pub implementation_id: conduit_core::ImplementationId,
    pub artifact_id: conduit_core::ArtifactId,
    pub execution_profile_id: conduit_core::ExecutionProfileId,
}
impl OfferDescription {
    pub(super) fn owned(offer: conduit_core::CapabilityOffer) -> Self {
        Self {
            inputs: offer.inputs,
            outputs: offer.outputs,
            semantic_contract: offer.semantic_contract,
            host_calls: offer.host_calls,
            realization_properties: offer.realization_properties,
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            execution_profile_id: offer.implementation.execution_profile_id,
        }
    }
    fn cloned(offer: &conduit_core::CapabilityOffer) -> Self {
        Self {
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            semantic_contract: offer.semantic_contract.clone(),
            host_calls: offer.host_calls.clone(),
            realization_properties: offer.realization_properties.clone(),
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            implementation_id: offer.implementation.implementation_id.clone(),
            artifact_id: offer.implementation.artifact_id.clone(),
            execution_profile_id: offer.implementation.execution_profile_id.clone(),
        }
    }
}
impl<'a> PlanningHosts<'a> {
    pub(super) fn access(&self) -> crate::offer_access::OfferAccess<'_> {
        match self {
            Self::Borrowed(h) => crate::offer_access::OfferAccess::Ordinary(h),
            Self::Owned(h) => crate::offer_access::OfferAccess::Ordinary(h),
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => crate::offer_access::OfferAccess::Indexed(h),
        }
    }
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = &HostAdvertisement> + use<'_, 'a> {
        let len = match self {
            Self::Borrowed(h) => h.len(),
            Self::Owned(h) => h.len(),
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => h.len(),
        };
        (0..len).map(move |index| match self {
            Self::Borrowed(h) => &h[index],
            Self::Owned(h) => &h[index],
            #[cfg(feature = "indexed-source")]
            Self::Indexed(h) => h[index].native,
        })
    }
    pub(super) fn capability(
        &self,
        host: &HostId,
        id: &CapabilityId,
    ) -> Result<alloc::borrow::Cow<'_, conduit_core::CapabilityOffer>, PlannerError> {
        let access = self.access();
        let index = access.host_index(host, true)?;
        access.find(index, id)
    }
    pub(super) fn validate_placements(
        &self,
        gears: &crate::planning_input::PlanningGears<'_>,
        placements: &PlacementChoices,
    ) -> Result<(), PlannerError> {
        match self {
            Self::Borrowed(h) => {
                crate::language_coverage::validate_placements(gears, h, placements)
            }
            Self::Owned(h) => crate::language_coverage::validate_placements(gears, h, placements),
            #[cfg(feature = "indexed-source")]
            Self::Indexed(_) => self.validate_indexed_placements(gears, placements),
        }
    }
    #[cfg(feature = "indexed-source")]
    fn validate_indexed_placements(
        &self,
        gears: &crate::planning_input::PlanningGears<'_>,
        placements: &PlacementChoices,
    ) -> Result<(), PlannerError> {
        let access = self.access();
        for host in 0..access.len() {
            for index in 0..access.count(host)? {
                let offer = access.offer(host, index)?;
                if !conduit_core::valid_realization_properties(&offer.realization_properties) {
                    return Err(PlannerError::InvalidRealizationProperties(
                        offer.capability_id.clone(),
                    ));
                }
            }
        }
        for gear in gears.iter() {
            let gear = gear?;
            let choice = placements
                .by_gear
                .get(&gear.gear_id)
                .ok_or_else(|| PlannerError::MissingPlacement(gear.gear_id.as_str().into()))?;
            let host = access.host_index(&choice.host_id, false)?;
            let offer = access.find(host, &choice.capability_id)?;
            crate::validate_operation_capability(&gear, &offer)?;
            crate::language_coverage::validate_selected(&gear, access.host(host), &offer)?;
        }
        Ok(())
    }
    pub(super) fn validate_resources(&self) -> Result<(), PlannerError> {
        let indexed = match self {
            #[cfg(feature = "indexed-source")]
            Self::Indexed(_) => true,
            _ => false,
        };
        if !indexed {
            for host in self.iter() {
                crate::validate_host_resources(host)?;
            }
            return Ok(());
        }
        let access = self.access();
        for index in 0..access.len() {
            crate::validate_host_resources_with_capabilities(access.host(index).into(), |id| {
                for offer_index in 0..access.count(index)? {
                    if access.offer(index, offer_index)?.capability_id.as_str() == id {
                        return Ok(true);
                    }
                }
                Ok(false)
            })?;
        }
        Ok(())
    }

    pub(super) fn retiring(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
    pub(super) fn retain_selected(&mut self, placements: &PlacementChoices) {
        let Self::Owned(hosts) = self else { return };
        for host in hosts {
            host.capabilities.retain(|offer| {
                placements.by_gear.values().any(|choice| {
                    choice.host_id == host.host_id && choice.capability_id == offer.capability_id
                })
            });
            host.capabilities.shrink_to_fit();
        }
    }
    pub(super) fn description(
        &mut self,
        host: &HostId,
        capability: &CapabilityId,
        last_use: bool,
    ) -> OfferDescription {
        let Self::Owned(hosts) = self else {
            let offer = self
                .iter()
                .rev()
                .find(|h| &h.host_id == host)
                .expect("admitted host")
                .capabilities
                .iter()
                .find(|c| &c.capability_id == capability)
                .expect("admitted capability");
            return OfferDescription::cloned(offer);
        };
        let offer = hosts
            .iter_mut()
            .rev()
            .find(|h| &h.host_id == host)
            .expect("admitted host")
            .capabilities
            .iter_mut()
            .find(|c| &c.capability_id == capability)
            .expect("admitted capability");
        macro_rules! take {
            ($field:ident) => {
                if last_use {
                    core::mem::take(&mut offer.$field)
                } else {
                    offer.$field.clone()
                }
            };
        }
        macro_rules! identity {
            ($field:ident) => {
                if last_use {
                    core::mem::replace(&mut offer.$field, "".into())
                } else {
                    offer.$field.clone()
                }
            };
        }
        macro_rules! implementation {
            ($field:ident) => {
                if last_use {
                    core::mem::replace(&mut offer.implementation.$field, "".into())
                } else {
                    offer.implementation.$field.clone()
                }
            };
        }
        let description = OfferDescription {
            inputs: take!(inputs),
            outputs: take!(outputs),
            semantic_contract: take!(semantic_contract),
            host_calls: take!(host_calls),
            realization_properties: take!(realization_properties),
            kind_id: identity!(kind_id),
            kind_contract_revision: identity!(kind_contract_revision),
            implementation_id: implementation!(implementation_id),
            artifact_id: implementation!(artifact_id),
            execution_profile_id: implementation!(execution_profile_id),
        };
        if last_use {
            offer.startup_parameters = Vec::new();
            offer.resource_requirements = Vec::new();
            offer.authority_requirements = Vec::new();
            offer.shorthand = None;
            offer.state_retention = None;
        }
        description
    }
}
