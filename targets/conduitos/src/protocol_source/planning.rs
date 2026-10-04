//! Publish retained pure Backs and derive exact queues from selected offers.
use super::*;
use alloc::collections::BTreeMap;
use conduit_core::{CapabilityId, ConfigurationValue, GearId, HostAdvertisement, PortDirection};
use conduit_planner::{
    ConnectionEndpoints, ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices,
};
use conduit_plot::ExpandedAuthoringPlot;

pub struct ProtocolQueueLimits {
    pub connections: BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    pub boundaries: BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
}

impl PreparedProtocolSource {
    pub fn expand(&self, entry: &str) -> Result<ExpandedAuthoringPlot, ProtocolSourceRefusal> {
        conduit_plot::expand_canonical_plot_for_authoring(&self.checked, entry, &self.profile)
            .map_err(ProtocolSourceRefusal::Expansion)
    }

    fn validate_expanded(
        &self,
        expanded: &ExpandedAuthoringPlot,
    ) -> Result<(), ProtocolSourceRefusal> {
        expanded
            .expanded
            .validate_expansion()
            .map_err(ProtocolSourceRefusal::Expansion)?;
        if expanded.expanded.source_document_id != self.checked.source_document_id
            || !self.checked.plots.iter().any(|plot| {
                plot.name == expanded.expanded.name
                    && plot.checked_plot_id == expanded.expanded.checked_plot_id
            })
        {
            return Err(ProtocolSourceRefusal::Offer);
        }
        Ok(())
    }

    /// Only code retained by this preparation is added. Native bases, resources,
    /// authority, Host and Boot remain exactly the caller's native truth.
    pub fn publish_pure_backs(
        &self,
        expanded: &ExpandedAuthoringPlot,
        host: &mut HostAdvertisement,
    ) -> Result<(), ProtocolSourceRefusal> {
        use ProtocolSourceRefusal as Error;
        self.validate_expanded(expanded)?;
        let mut offers = BTreeMap::<CapabilityId, CapabilityOffer>::new();
        let mut retain = |offer: CapabilityOffer| -> Result<(), Error> {
            if let Some(previous) = offers.get(&offer.capability_id) {
                if previous != &offer {
                    return Err(Error::Offer);
                }
            } else {
                offers.insert(offer.capability_id.clone(), offer);
            }
            Ok(())
        };
        for offer in &self.capabilities {
            retain(offer.clone())?;
        }
        for gear in &expanded.expanded.gears {
            if gear.configuration.is_empty() {
                continue;
            }
            let [entry] = gear.configuration.as_slice() else {
                return Err(Error::Offer);
            };
            let ConfigurationValue::Text(encoded) = &entry.value else {
                return Err(Error::Offer);
            };
            let temporal = gear
                .checked_front()
                .inputs()
                .first()
                .ok_or(Error::Offer)?
                .temporal;
            let offer = match entry.key.as_str() {
                "program" => {
                    let program =
                        conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                            .map_err(|_| Error::Offer)?;
                    crate::expression_host_call::offer(&program, temporal)
                        .map_err(|_| Error::Offer)?
                }
                "selector" => {
                    let selector = conduit_core::StructuredSelector::from_canonical_hex(encoded)
                        .map_err(|_| Error::Offer)?;
                    crate::structured_selector_host_call::offer(&selector, temporal)
                        .map_err(|_| Error::Offer)?
                }
                _ => return Err(Error::Offer),
            };
            retain(offer)?;
        }
        let mut additions = Vec::new();
        for offer in offers.into_values() {
            let mut existing = host
                .capabilities
                .iter()
                .filter(|existing| existing.capability_id == offer.capability_id);
            if let Some(present) = existing.next() {
                if present != &offer || existing.next().is_some() {
                    return Err(Error::Offer);
                }
            } else {
                additions.push(offer);
            }
        }
        host.capabilities.extend(additions);
        Ok(())
    }

    /// Queue policy is one item per Cord, within both exact selected Backs.
    /// Selection itself remains ordinary planner truth supplied by the caller.
    pub fn queue_limits(
        &self,
        expanded: &ExpandedAuthoringPlot,
        hosts: &[HostAdvertisement],
        placements: &PlacementChoices,
    ) -> Result<ProtocolQueueLimits, ProtocolSourceRefusal> {
        use ProtocolSourceRefusal as Error;
        self.validate_expanded(expanded)?;
        let offer = |gear: &GearId| -> Result<&CapabilityOffer, Error> {
            let choice = placements.by_gear.get(gear).ok_or(Error::Offer)?;
            let mut matches = hosts
                .iter()
                .filter(|host| host.host_id == choice.host_id)
                .flat_map(|host| &host.capabilities)
                .filter(|offer| offer.capability_id == choice.capability_id);
            let offer = matches.next().ok_or(Error::Offer)?;
            if matches.next().is_some() {
                return Err(Error::Offer);
            }
            Ok(offer)
        };
        let mut boundaries = BTreeMap::new();
        for (direction, bindings) in [
            (PortDirection::Input, &expanded.input_bindings),
            (PortDirection::Output, &expanded.output_bindings),
        ] {
            for binding in bindings {
                let selected = offer(&binding.gear_id)?;
                let key = ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                };
                let limit = ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: selected.limits.max_queue_bytes,
                };
                boundaries
                    .entry(key)
                    .and_modify(|previous: &mut ConnectionQueueLimits| {
                        previous.byte_capacity = previous.byte_capacity.min(limit.byte_capacity);
                    })
                    .or_insert(limit);
            }
        }
        let mut connections = BTreeMap::new();
        for cord in &expanded.expanded.connections {
            let bytes = offer(&cord.source_gear_id)?
                .limits
                .max_queue_bytes
                .min(offer(&cord.sink_gear_id)?.limits.max_queue_bytes);
            connections.insert(
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
        Ok(ProtocolQueueLimits {
            connections,
            boundaries,
        })
    }
}

#[cfg(test)]
mod tests;
