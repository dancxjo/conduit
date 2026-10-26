//! Bind retained software to one locally verified fragment. This establishes no
//! native possession, memory admission, Body start or permission to execute.
use super::*;
use conduit_plan_lowering::local_tables::{VerifiedLocalKernelTables, VerifiedLocalLowering};
use conduit_plot::flat_image::indexed::IndexedSourceImage;

pub struct PreparedLocalAppliance<'a, V> {
    pub tables: VerifiedLocalKernelTables<'a, NODES, CORDS, PORTS, NODES, CORDS, NODES>,
    pub drivers: [ApplianceBack; NODES],
    pub selectors: [Option<PreparedCaptureSelector>; NODES],
    pub values: V,
    pub roles: ResidentRoles,
}
impl<V: ValueStorage> PreparedResidentSoftware<V> {
    /// Check against the independently authenticated Make image and actual
    /// initialized offer owner. Decodes one Source gear/offer at a time. Caller
    /// supplies Root's actual current Host/Boot/offer epoch, not packet claims.
    pub fn check_local(
        &self,
        verified: &VerifiedLocalLowering<'_>,
        image: &IndexedSourceImage<'_>,
        offers: &[ResidentSoftwareOffer<'_>],
        scope: (&c::HostId, &c::BootId, c::OfferGeneration),
    ) -> Result<(), NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        let (host, boot, generation) = scope;
        let lowered = verified.lowered();
        let (plan, _) = verified.preparation_scope();
        let metadata = plan.preparation_metadata();
        let fragment = plan
            .fragment(&lowered.identity.fragment_id)
            .ok_or(E::Configuration)?;
        let selected = fragment.lowering_metadata();
        let identity = image.identity().map_err(|_| E::Configuration)?;
        if ResidentSourceIdentity::from_expanded(&identity.expanded_plot_id)
            != self.construction.source
            || metadata.source_document_id != identity.source_document_id
            || metadata.checked_plot_id != identity.checked_plot_id
            || metadata.expanded_plot_id != identity.expanded_plot_id
            || selected.host_id != *host
            || selected.boot_id != *boot
            || selected.offer_generation != generation
            || generation.0 == 0
            || image.gear_count() != NODES
            || selected.placements.len() != NODES
            || lowered.nodes.len() != NODES
            || self.drivers.iter().any(Option::is_none)
        {
            return Err(E::Configuration);
        }
        {
            let source = image.metadata().map_err(|_| E::Configuration)?;
            if selected.connections.len() != source.connections().len() {
                return Err(E::Configuration);
            }
            for cord in source.connections() {
                let matches = selected
                    .connections
                    .iter()
                    .filter(|selected_cord| {
                        let gear_id = |placement: &c::PlacementId| {
                            selected
                                .placements
                                .iter()
                                .find(|gear| &gear.placement_id == placement)
                                .map(|gear| &gear.gear_id)
                        };
                        gear_id(&selected_cord.source_placement_id) == Some(&cord.source_gear_id)
                            && gear_id(&selected_cord.sink_placement_id) == Some(&cord.sink_gear_id)
                            && selected_cord.source_port_id == cord.source_port_id
                            && selected_cord.sink_port_id == cord.sink_port_id
                            && selected_cord.value_kind == cord.value_kind
                            && selected_cord.track == cord.track
                            && selected_cord.temporal == cord.temporal
                    })
                    .count();
                if matches != 1 {
                    return Err(E::Configuration);
                }
            }
        }
        // Validate descriptors against this actual initialized owner before using
        // any of them as expected software facts.
        let spec = ResidentSoftwareSpec {
            cadence: conduit_time::TickConfiguration {
                count: self.construction.count,
                period_ms: self.construction.period_ms,
            },
            source: self.construction.source,
            roles: self.construction.roles,
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: self
                    .construction
                    .computations
                    .iter()
                    .map(|r| r.text_bytes)
                    .max()
                    .unwrap_or(0),
                maximum_total_text_bytes: self
                    .construction
                    .computations
                    .iter()
                    .map(|r| u32::from(r.text_bytes))
                    .sum(),
                requirements: &self.construction.computations,
            },
        };
        self.source_offers(&spec, offers)?;
        for node in 0..NODES {
            let source = image.gear(node).map_err(|_| E::Configuration)?;
            let gear = selected
                .placements
                .iter()
                .find(|g| g.gear_id == source.gear_id)
                .ok_or(E::Configuration)?;
            if lowered.identity.node_for_placement(&gear.placement_id)
                != Some(k::NodeId(node as u16))
                || gear.kind_id != source.kind_id
                || gear.host_id != *host
                || gear.boot_id != *boot
                || gear.offer_generation != generation
                || !fragment.configuration_matches(&gear.placement_id, &source.configuration)
            {
                return Err(E::Configuration);
            }
            let roles = self.construction.roles;
            for (role, kind) in [
                (roles.pulse, conduit_pulse_capture::PULSE_READ_KIND),
                (roles.tick, conduit_time::TICK_KIND),
                (roles.trigger, conduit_time::TICK_TRIGGER_KIND),
                (roles.observation, crate::observation::KIND),
            ]
            .into_iter()
            .chain(
                roles
                    .publications
                    .into_iter()
                    .zip(PublicationChannel::ALL)
                    .map(|(node, channel)| (node, channel.kind_id())),
            ) {
                if (usize::from(role.0) == node) != (source.kind_id.as_str() == kind) {
                    return Err(E::Roles);
                }
            }
            if let Some(expected) = self
                .construction
                .computations
                .iter()
                .find(|r| usize::from(r.node) == node)
            {
                if source.configuration.len() != 1 {
                    return Err(E::Configuration);
                }
                let entry = &source.configuration[0];
                let c::ConfigurationValue::Text(text) = &entry.value else {
                    return Err(E::Configuration);
                };
                if c::AssignedComputationRequirement::from_text(
                    node as u16,
                    &entry.key,
                    text.as_str(),
                )
                .map_err(|_| E::Configuration)?
                    != *expected
                {
                    return Err(E::Configuration);
                }
            }
            if node as u16 == roles.tick.0 {
                let expected = [
                    c::ConfigurationEntry {
                        key: "count".into(),
                        value: c::ConfigurationValue::U64(self.construction.count),
                    },
                    c::ConfigurationEntry {
                        key: "period-ms".into(),
                        value: c::ConfigurationValue::U64(self.construction.period_ms),
                    },
                ];
                if source.configuration.len() != expected.len()
                    || expected
                        .iter()
                        .any(|entry| !source.configuration.contains(entry))
                {
                    return Err(E::Configuration);
                }
            }
            if node as u16 == self.construction.roles.pulse.0
                || node as u16 == self.construction.roles.tick.0
            {
                // Actual native providers are independently bound by Root.
                let kind = if node as u16 == self.construction.roles.pulse.0 {
                    conduit_pulse_capture::PULSE_READ_KIND
                } else {
                    conduit_time::TICK_KIND
                };
                if gear.kind_id.as_str() != kind {
                    return Err(E::Roles);
                }
                continue;
            }
            // Configuration/provenance is checked above. Retire its decoded
            // Source gear before decoding the independently retained offer.
            drop(source);
            let descriptor = offers
                .iter()
                .find(|o| usize::from(o.node) == node)
                .ok_or(E::Configuration)?;
            let offer = descriptor.decode()?;
            if gear.limits != offer.limits
                || gear.kind_id != offer.kind_id
                || gear.kind_contract_revision != offer.kind_contract_revision
                || gear.semantic_contract != offer.semantic_contract
                || gear.inputs != offer.inputs
                || gear.outputs != offer.outputs
                || gear.capability_id != offer.capability_id
                || gear.execution_profile_id != offer.implementation.execution_profile_id
                || gear.implementation_id != offer.implementation.implementation_id
                || gear.artifact_id != offer.implementation.artifact_id
                || gear.host_calls != offer.host_calls
                || gear.base.is_some()
                || !gear.resources.is_empty()
                || !gear.authority.is_empty()
                || !gear.pool_references.is_empty()
                || !gear.terminal_transductions.is_empty()
                || !gear.realization_properties.is_empty()
                || !gear.realization_characteristics.is_empty()
            {
                return Err(E::Configuration);
            }
        }
        Ok(())
    }
    /// Every refusal precedes transfer. No evaluator/value store is rebuilt.
    /// Caller must admit this finite destination plus table preparation scratch.
    pub fn bind_local_retained<'a>(
        slot: &mut Option<Box<Self>>,
        verified: &'a VerifiedLocalLowering<'a>,
        image: &IndexedSourceImage<'_>,
        offers: &[ResidentSoftwareOffer<'_>],
        scope: (&c::HostId, &c::BootId, c::OfferGeneration),
    ) -> Result<PreparedLocalAppliance<'a, V>, NativePreparationRefusal> {
        slot.as_ref()
            .ok_or(NativePreparationRefusal::Initialization)?
            .check_local(verified, image, offers, scope)?;
        let tables = VerifiedLocalKernelTables::from_verified_lowering(verified)
            .map_err(NativePreparationRefusal::LocalTables)?;
        let Self {
            construction,
            drivers,
            selectors,
            values,
        } = *slot.take().expect("preflighted local software");
        Ok(PreparedLocalAppliance {
            tables,
            drivers: (*drivers).map(|d| d.expect("preflighted complete drivers")),
            selectors: *selectors,
            values,
            roles: construction.roles,
        })
    }
}
