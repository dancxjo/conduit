//! Explicit local OS planning intent, never execution or native possession.
//! Current offers must be independently authenticated before selection; actual
//! local Fore owners are checked again by the executing Host at admission.
use conduit_body::ResidentPlot;
use conduit_core as c;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalForeAdapterIntent {
    pub entry: String,
    pub host_id: c::HostId,
    pub configuration_id: String,
    pub ports: Vec<LocalForePortIntent>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalForePortIntent {
    pub front_port_id: c::PortId,
    pub direction: c::PortDirection,
    pub track: c::ConnectionTrack,
    pub value_kind: c::KindId,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}
struct Entry {
    host: c::HostId,
    boot: c::BootId,
    generation: c::OfferGeneration,
    configuration: c::HostProfileId,
    boundaries: BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
}
pub(crate) struct SelectedLocalForeSet {
    root: (c::HostId, c::BootId, c::OfferGeneration),
    entries: BTreeMap<ResidentPlot, Entry>,
}
impl SelectedLocalForeSet {
    /// Called only after the affine local File authorization verifies its
    /// original Root and independently checked complete Source selection.
    pub(crate) fn prepare(
        root: &c::HostAdvertisement,
        plots: &[conduit_plot::ExpandedAuthoringPlot],
        current: &[c::HostAdvertisement],
        intents: &[LocalForeAdapterIntent],
    ) -> Result<Self, String> {
        if intents.len() > conduit_body::MAX_BODY_PLOTS {
            return Err("local adapter selection exceeds complete workset capacity".into());
        }
        let mut entries = BTreeMap::new();
        for intent in intents {
            let plot = plots
                .iter()
                .find(|p| p.expanded.name == intent.entry)
                .ok_or("selected local adapter entry is absent from checked Source")?;
            let mut hosts = current.iter().filter(|h| h.host_id == intent.host_id);
            let host = hosts
                .next()
                .ok_or("selected local adapter Host has no current offer")?;
            if hosts.next().is_some()
                || host.profile.as_str() != intent.configuration_id
                || host.offer_generation.0 == 0
                || intent.ports.len() > 32
            {
                return Err(
                    "selected local adapter differs from current Host configuration".into(),
                );
            }
            let mut boundaries = BTreeMap::new();
            for port in &intent.ports {
                let bindings = match port.direction {
                    c::PortDirection::Input => &plot.input_bindings,
                    c::PortDirection::Output => &plot.output_bindings,
                };
                let binding = bindings
                    .iter()
                    .find(|b| b.front_port_id == port.front_port_id && b.track == port.track)
                    .ok_or("selected local adapter Fore is absent from checked Source")?;
                let gear = plot
                    .expanded
                    .gears
                    .iter()
                    .find(|g| g.gear_id == binding.gear_id)
                    .ok_or("selected local adapter Gear is absent")?;
                let gear_ports = match port.direction {
                    c::PortDirection::Input => &gear.inputs,
                    c::PortDirection::Output => &gear.outputs,
                };
                let descriptor = gear_ports
                    .iter()
                    .find(|p| p.port_id == binding.gear_port_id)
                    .ok_or("selected local adapter Gear Port is absent")?;
                let empty = c::KindId::from(c::EMPTY_INFO_ID);
                let kind = match port.track {
                    c::ConnectionTrack::Payload => &descriptor.value_kind,
                    c::ConnectionTrack::AbnormalTerminal => descriptor
                        .abnormal_kind
                        .as_ref()
                        .ok_or("selected local adapter has no abnormal contract")?,
                    c::ConnectionTrack::NormalClose | c::ConnectionTrack::Quiescence => &empty,
                };
                if kind != &port.value_kind
                    || port.item_capacity == 0
                    || port.item_capacity > 32
                    || port.byte_capacity == 0
                    || port.byte_capacity > 16384
                    || boundaries
                        .insert(
                            ForeBoundaryKey {
                                direction: port.direction,
                                front_port_id: port.front_port_id.clone(),
                                track: port.track,
                            },
                            ConnectionQueueLimits {
                                item_capacity: port.item_capacity,
                                byte_capacity: port.byte_capacity,
                            },
                        )
                        .is_some()
                {
                    return Err("selected local adapter contract or finite capacity differs".into());
                }
            }
            if plot.input_bindings.iter().any(|input| {
                !boundaries.contains_key(&ForeBoundaryKey {
                    direction: c::PortDirection::Input,
                    front_port_id: input.front_port_id.clone(),
                    track: input.track,
                })
            }) {
                return Err("selected local adapter leaves a checked input unbound".into());
            }
            let resident = ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            );
            if entries
                .insert(
                    resident,
                    Entry {
                        host: host.host_id.clone(),
                        boot: host.boot_id.clone(),
                        generation: host.offer_generation,
                        configuration: host.profile.clone(),
                        boundaries,
                    },
                )
                .is_some()
            {
                return Err("selected local adapter entry is duplicated".into());
            }
        }
        Ok(Self {
            root: (
                root.host_id.clone(),
                root.boot_id.clone(),
                root.offer_generation,
            ),
            entries,
        })
    }
    pub(crate) fn verify(
        &self,
        root: &c::HostAdvertisement,
        current: &[c::HostAdvertisement],
    ) -> Result<(), String> {
        if self.root
            != (
                root.host_id.clone(),
                root.boot_id.clone(),
                root.offer_generation,
            )
            || self.entries.values().any(|entry| {
                !current.iter().any(|h| {
                    h.host_id == entry.host
                        && h.boot_id == entry.boot
                        && h.offer_generation == entry.generation
                        && h.profile == entry.configuration
                })
            })
        {
            return Err(
                "local adapter selection lost its original Root or current target offer".into(),
            );
        }
        Ok(())
    }
    pub(crate) fn boundaries(
        &self,
        plot: &ResidentPlot,
    ) -> Option<&BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>> {
        self.entries.get(plot).map(|entry| &entry.boundaries)
    }
    pub(crate) fn target(&self, plot: &ResidentPlot) -> Option<&c::HostId> {
        self.entries.get(plot).map(|entry| &entry.host)
    }
}
