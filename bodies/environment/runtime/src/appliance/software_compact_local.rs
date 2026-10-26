//! Exact selection from a locally verified Plan and independently checked Make
//! Source. This transfers pure owners only, never the native output provider.
use super::*;
use conduit_plan_lowering::local_tables::VerifiedLocalLowering;
use conduit_plot::flat_image::indexed::IndexedSourceImage;

impl CompactOutputSoftware<'_> {
    pub fn check_local(
        &self,
        verified: &VerifiedLocalLowering<'_>,
        image: &IndexedSourceImage<'_>,
        scope: (&c::HostId, &c::BootId, c::OfferGeneration),
    ) -> Result<(), SoftwareRefusal> {
        self.local_indices(verified, image, scope).map(|_| ())
    }
    pub(crate) fn take_local(
        &mut self,
        verified: &VerifiedLocalLowering<'_>,
        image: &IndexedSourceImage<'_>,
        scope: (&c::HostId, &c::BootId, c::OfferGeneration),
    ) -> Result<[Option<PreparedSoftwareOperation>; 4], SoftwareRefusal> {
        let indices = self.local_indices(verified, image, scope)?;
        Ok(core::array::from_fn(|node| {
            indices[node].map(|index| {
                self.operations[index]
                    .prepared
                    .take()
                    .expect("complete exclusive preflight")
            })
        }))
    }
    fn local_indices(
        &self,
        verified: &VerifiedLocalLowering<'_>,
        image: &IndexedSourceImage<'_>,
        scope: (&c::HostId, &c::BootId, c::OfferGeneration),
    ) -> Result<[Option<usize>; 4], SoftwareRefusal> {
        use SoftwareRefusal as E;
        let (host, boot, generation) = scope;
        let (plan, lowered) = verified.preparation_scope();
        let identity = image.identity().map_err(|_| E::Source)?;
        let fragment = plan
            .fragment(&lowered.identity.fragment_id)
            .ok_or(E::Selection)?;
        let selected = fragment.lowering_metadata();
        if host != &self.host
            || boot != &self.boot
            || generation.0 == 0
            || self.generation != Some(generation)
            || identity != self.source
            || selected.source_document_id != identity.source_document_id
            || selected.checked_plot_id != identity.checked_plot_id
            || selected.expanded_plot_id != identity.expanded_plot_id
            || selected.host_id != *host
            || selected.boot_id != *boot
            || selected.offer_generation != generation
            || plan.fragment_count() != 1
            || image.gear_count() != 4
            || selected.placements.len() != 4
            || lowered.nodes.len() != 4
        {
            return Err(E::Selection);
        }
        // Bind the selected routing to the checked Source rather than trusting
        // a caller's claimed Source ID on a different, self-consistent Plan.
        {
            let source = image.metadata().map_err(|_| E::Source)?;
            if selected.connections.len() != source.connections().len() {
                return Err(E::Selection);
            }
            let gear = |id: &c::PlacementId| {
                selected
                    .placements
                    .iter()
                    .find(|p| &p.placement_id == id)
                    .map(|p| &p.gear_id)
            };
            for cord in source.connections() {
                if selected
                    .connections
                    .iter()
                    .filter(|c| {
                        gear(&c.source_placement_id) == Some(&cord.source_gear_id)
                            && gear(&c.sink_placement_id) == Some(&cord.sink_gear_id)
                            && c.source_port_id == cord.source_port_id
                            && c.sink_port_id == cord.sink_port_id
                            && c.value_kind == cord.value_kind
                            && c.track == cord.track
                            && c.temporal == cord.temporal
                    })
                    .count()
                    != 1
                {
                    return Err(E::Selection);
                }
            }
        }
        let mut indices = [None; 4];
        let mut writes = 0;
        for (node, index) in indices.iter_mut().enumerate() {
            let source = image.gear(node).map_err(|_| E::Source)?;
            let gear = selected
                .placements
                .iter()
                .find(|g| g.gear_id == source.gear_id)
                .ok_or(E::Selection)?;
            if lowered.identity.node_for_placement(&gear.placement_id)
                != Some(conduit_kernel::NodeId(node as u16))
                || gear.kind_id != source.kind_id
                || gear.host_id != *host
                || gear.boot_id != *boot
                || gear.offer_generation != generation
                || !fragment.configuration_matches(&gear.placement_id, &source.configuration)
            {
                return Err(E::Selection);
            }
            if source.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND {
                writes += 1;
                continue;
            }
            let [configuration] = source.configuration.as_slice() else {
                return Err(E::Selection);
            };
            let c::ConfigurationValue::Text(text) = &configuration.value else {
                return Err(E::Selection);
            };
            let found = self
                .operations
                .iter()
                .position(|op| {
                    op.node[..16] == c::AssignedIdentity::from_text(gear.kind_id.as_str()).0
                        && op.node[16..32]
                            == c::AssignedIdentity::from_text(gear.implementation_id.as_str()).0
                        && op.node[32..]
                            == c::AssignedIdentity::from_text(gear.artifact_id.as_str()).0
                })
                .ok_or(E::Selection)?;
            let op = &self.operations[found];
            if op.key != c::AssignedIdentity::from_text(&configuration.key)
                || op.configuration != text
            {
                return Err(E::Selection);
            }
            if op.prepared.is_none() {
                return Err(E::AlreadySelected);
            }
            // Do not keep a decoded Source gear and advertised schema together.
            drop(source);
            let offer: c::CapabilityOffer =
                serde_json::from_slice(op.offer).map_err(|_| E::Selection)?;
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
                return Err(E::Selection);
            }
            *index = Some(found);
        }
        if writes != 1
            || indices.iter().flatten().count() != 3
            || (0..4).any(|a| (a + 1..4).any(|b| indices[a].is_some() && indices[a] == indices[b]))
        {
            return Err(E::Selection);
        }
        Ok(indices)
    }
}
