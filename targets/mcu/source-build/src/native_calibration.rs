//! Build-only descriptions for checked native-equipment Hosts. These are
//! inert Planner inputs. They do not initialize hardware or grant a runtime Play.
use crate::{template_calibration as t, workset};
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_host_make as h;
use conduit_planner as p;
use std::collections::BTreeMap;

pub struct Slot<'a> {
    pub base: &'a str,
    pub pool: &'a str,
    pub implementation: &'a str,
}
pub struct NativeDescription<'a> {
    pub slot: Slot<'a>,
    pub class: &'a str,
    pub mechanism: &'a str,
    pub definition: c::CapabilityOffer,
}
pub struct Bounds {
    pub image: e::EmbeddedImageBounds,
    pub computations: e::EmbeddedComputationBounds,
    pub maxima: c::AssignedPlanMaxima,
    pub maximum_fore_ports: usize,
    pub maximum_offer_image_bytes: usize,
}
impl Bounds {
    /// Finite artifact ceilings from the checked Host. These remain parser/
    /// lowering ceilings; original runtime admission must fund actual backing.
    pub fn for_workset(
        checked: &h::CheckedHostConfiguration,
        workset: &workset::Workset,
        computations: e::EmbeddedComputationBounds,
    ) -> Result<Self, String> {
        let limits = &checked.configuration().limits;
        let preparation = checked
            .configuration()
            .preparation
            .as_ref()
            .ok_or("missing native preparation policy")?;
        let encoded = preparation
            .maximum_assignment_bytes
            .try_into()
            .map_err(|_| "assignment exceeds binary format")?;
        let runtime = preparation
            .maximum_runtime_state_bytes
            .try_into()
            .map_err(|_| "runtime exceeds binary format")?;
        let bytes = u32::try_from(limits.buffered_bytes)
            .map_err(|_| "buffer ceiling exceeds binary format")?;
        // Static topology is exact checked Source material, not a promise of
        // active execution. Actual storage still needs original whole admission.
        let nodes = workset
            .entries
            .iter()
            .map(|e| e.plot.expanded.gears.len())
            .max()
            .ok_or("empty native workset")?;
        let cords = workset
            .entries
            .iter()
            .map(|e| {
                e.plot
                    .expanded
                    .connections
                    .len()
                    .checked_add(e.plot.input_bindings.len())
                    .and_then(|n| n.checked_add(e.plot.output_bindings.len()))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or("Cord table count overflow")?
            .into_iter()
            .max()
            .ok_or("empty native workset")?;
        let fronts = workset
            .entries
            .iter()
            .map(|e| e.plot.input_bindings.len() + e.plot.output_bindings.len())
            .max()
            .unwrap_or(0);
        let routes = cords
            .checked_add(fronts)
            .ok_or("route table count overflow")?;
        let calls = nodes;
        let signs = u16::try_from(limits.evidence_items)
            .map_err(|_| "Sign ceiling exceeds binary format")?;
        Ok(Self {
            image: e::EmbeddedImageBounds {
                maximum_nodes: nodes,
                maximum_cords: cords,
                maximum_routes: routes,
                maximum_route_targets: routes,
                maximum_host_calls: calls,
                maximum_resources: calls,
                maximum_sign_expectations: workset
                    .entries
                    .iter()
                    .map(|entry| {
                        entry
                            .plot
                            .expanded
                            .gears
                            .len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(entry.plot.expanded.connections.len()))
                            .and_then(|n| n.checked_add(2))
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or("Sign table count overflow")?
                    .into_iter()
                    .max()
                    .ok_or("empty native workset")?,
                maximum_configuration_entries: nodes
                    .checked_mul(8)
                    .ok_or("configuration bound overflow")?,
                maximum_ports_per_node: 4,
                maximum_remote_endpoints: 0,
                maximum_cord_value_slots: u16::try_from(limits.queue_items)
                    .map_err(|_| "queue ceiling exceeds binary format")?,
                maximum_cord_value_bytes: bytes,
                maximum_sign_items: signs,
                maximum_sign_bytes: bytes,
            },
            // Caller-reviewed immutable codebook ceilings are independent of
            // queue/heap custody; BSP flash qualification remains separate.
            computations,
            maxima: c::AssignedPlanMaxima {
                encoded_bytes: encoded,
                runtime_state_bytes: runtime,
                counts: [u8::MAX; c::ASSIGNED_PLAN_COUNT_KINDS],
            },
            maximum_fore_ports: usize::try_from(limits.operation_slots)
                .map_err(|_| "operation ceiling exceeds Host tooling")?,
            maximum_offer_image_bytes: 24 * 1024,
        })
    }
}
pub struct Input<'a> {
    pub checked: &'a h::CheckedHostConfiguration,
    pub image: &'a h::HostImage,
    pub catalog: &'a h::MakeCatalog,
    pub shared_resources: &'a [c::ResourceOffer],
    pub native: &'a [NativeDescription<'a>],
    pub other: &'a [c::CapabilityOffer],
    pub resident: &'a [e::PreparedResidentExpression],
    pub bounds: Bounds,
}
/// Calibrate exact local topology with checked finite construction. No native
/// descriptive Boot, original-grant text or placeholder Host identity is emitted.
pub fn compile(workset: &workset::Workset, input: Input<'_>) -> Result<t::Calibration, String> {
    h::verify_configuration_image(input.checked, input.image, input.catalog)?;
    for resource in input.shared_resources {
        if !input.checked.configuration().resources.iter().any(|r| {
            r.id == resource.pool_id.as_str()
                && r.class == resource.class_id.as_str()
                && r.slots == resource.capacity_units
        }) {
            return Err(
                "shared calibration resource differs from checked physical construction".into(),
            );
        }
    }
    let mut host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fabrication/inert-native-description".into(),
        boot_id: "fabrication/inert-structural-calibration".into(),
        offer_generation: c::OfferGeneration(1),
        profile: input.checked.configuration_id().into(),
        bases: Vec::new(),
        resources: input.shared_resources.to_vec(),
        capabilities: input
            .resident
            .iter()
            .map(|r| r.offer_description().clone())
            .collect(),
        planner_capabilities: Vec::new(),
    };
    let mut grants = Vec::new();
    for description in input.native {
        let slot = &description.slot;
        let class = description.class;
        let offer = &description.definition;
        offer
            .validate_constructed_semantic_contract()
            .map_err(debug)?;
        if !input
            .checked
            .resolved_bases()
            .iter()
            .any(|(_, implementation)| implementation == slot.implementation)
            || offer.implementation.implementation_id.as_str() != slot.implementation
            || offer.implementation.artifact_id.as_str() != input.image.manifest.image_id
            || !offer
                .resource_requirements
                .iter()
                .any(|r| r.class_id.as_str() == class)
            || host.bases.iter().any(|b| b.base_id.as_str() == slot.base)
            || description.mechanism.is_empty()
            || slot.base.is_empty()
            || slot.pool.is_empty()
            || slot.implementation.is_empty()
            || host
                .resources
                .iter()
                .any(|r| r.pool_id.as_str() == slot.pool)
        {
            return Err("invalid or duplicate checked native calibration slot".into());
        }
        for (ordinal, authority) in offer.authority_requirements.iter().enumerate() {
            grants.push(c::AuthorityGrant {
                // Each distinct requirement needs a distinct descriptive identity.
                // These calibration grants are never emitted or used at execution.
                grant_id: format!("fabrication/inert-policy/{}/{ordinal}", slot.base).into(),
                contract_id: authority.contract_id.clone(),
                host_call_contract_id: authority.host_call_contract_id.clone(),
                subject_kind: authority.subject_kind.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                capability_id: offer.capability_id.clone(),
            });
        }
        host.bases.push(c::BaseProviderAdvertisement {
            base_id: slot.base.into(),
            provider_instance_id: slot.base.into(),
            provider_generation: 1,
            implementation_id: slot.implementation.into(),
            mechanism_family: description.mechanism.into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids: vec![offer.capability_id.clone()],
            resource_pool_ids: vec![slot.pool.into()],
        });
        host.resources.push(c::resource_offer(slot.pool, class, 1));
        host.capabilities.push(offer.clone());
    }
    let mut entries = Vec::new();
    for entry in &workset.entries {
        let software = crate::software::compile_with_types(
            &entry.plot.expanded,
            Some(&workset.checked_source),
            input.bounds.maximum_offer_image_bytes,
        )?;
        for offer in software.nodes.into_iter().filter_map(|n| n.offer) {
            insert(&mut host, offer)?;
        }
        for offer in input.other {
            if entry
                .plot
                .expanded
                .gears
                .iter()
                .any(|g| g.kind_id == offer.kind_id)
            {
                insert(&mut host, offer.clone())?;
            }
        }
    }
    // Core's Host resource contract has a canonical, strictly ordered pool
    // inventory. This changes no pool identity, capacity or per-node resource.
    host.resources
        .sort_unstable_by(|a, b| a.pool_id.cmp(&b.pool_id));
    host.bases
        .sort_unstable_by(|a, b| a.base_id.cmp(&b.base_id));
    for entry in &workset.entries {
        let plots = &entry.plot.expanded;
        if plots
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == conduit_time::TICK_KIND)
            .count()
            > 1
        {
            return Err(
                "multiple native clocks require distinct capability-instance selection".into(),
            );
        }
        let placements =
            p::default_expanded_placements(plots, std::slice::from_ref(&host)).map_err(debug)?;
        let offer = |gear: &c::GearId| -> Result<&c::CapabilityOffer, String> {
            let placement = placements
                .by_gear
                .get(gear)
                .ok_or("missing calibrated placement")?;
            host.capabilities
                .iter()
                .find(|o| o.capability_id == placement.capability_id)
                .ok_or("missing calibrated offer".into())
        };
        // Match the ordinary owner's queue policy exactly. These independent
        // immutable records must calibrate the signed Plan, not a private
        // compiler-only queue preference.
        let connections =
            p::default_expanded_connection_limits(plots, std::slice::from_ref(&host), &placements)
                .map_err(debug)?;
        let mut boundaries = BTreeMap::new();
        for (direction, bindings) in [
            (c::PortDirection::Input, &entry.plot.input_bindings),
            (c::PortDirection::Output, &entry.plot.output_bindings),
        ] {
            for binding in bindings {
                let location = match (direction, binding.track) {
                    (c::PortDirection::Input, c::ConnectionTrack::AbnormalTerminal) => {
                        c::FrontValueLocation::InputAbnormal(binding.gear_port_id.clone())
                    }
                    (c::PortDirection::Input, _) => {
                        c::FrontValueLocation::Input(binding.gear_port_id.clone())
                    }
                    (_, c::ConnectionTrack::AbnormalTerminal) => {
                        c::FrontValueLocation::OutputAbnormal(binding.gear_port_id.clone())
                    }
                    _ => c::FrontValueLocation::Output(binding.gear_port_id.clone()),
                };
                boundaries.insert(
                    p::ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    p::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: extent(offer(&binding.gear_id)?, &location)?,
                    },
                );
            }
        }
        entries.push(t::EntryLimits {
            entry: entry.name.clone(),
            connections,
            boundaries,
        });
    }
    let residents = input.resident.iter().collect::<Vec<_>>();
    t::compile(
        workset,
        t::Inputs {
            checked_host: input.checked,
            image: input.image,
            catalog: input.catalog,
            host: &host,
            authority_descriptions: &grants,
            protected_resource_descriptions: &[],
            resident: &residents,
            entries: &entries,
            internal_queue: p::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1,
            },
            image_bounds: input.bounds.image,
            computations: input.bounds.computations,
            maxima: input.bounds.maxima,
            maximum_fore_ports: input.bounds.maximum_fore_ports,
            maximum_offer_image_bytes: input.bounds.maximum_offer_image_bytes,
        },
    )
}
fn insert(host: &mut c::HostAdvertisement, offer: c::CapabilityOffer) -> Result<(), String> {
    if let Some(prior) = host
        .capabilities
        .iter()
        .find(|o| o.capability_id == offer.capability_id)
    {
        if prior != &offer {
            return Err("conflicting calibrated capability definition".into());
        }
    } else {
        host.capabilities.push(offer);
    }
    Ok(())
}
fn extent(offer: &c::CapabilityOffer, location: &c::FrontValueLocation) -> Result<u32, String> {
    let bytes = offer
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|v| &v.location == location)
        .map_or(offer.limits.max_queue_bytes, |v| v.contract.maximum_bytes);
    if bytes == 0 {
        return Ok(1);
    } // a finite Unit still owns its queue item
    Ok(bytes)
}
fn debug(e: impl core::fmt::Debug) -> String {
    format!("native structural calibration: {e:?}")
}

impl Bounds {
    /// Public exact parser/storage selection for the ordinary owner policy.
    /// These are compilation ceilings, never resource grants or Ready evidence.
    pub fn policy_json(&self, maximum_document_bytes: u32) -> serde_json::Value {
        let i = self.image;
        serde_json::json!({
            "image":{
                "maximum_nodes":i.maximum_nodes,"maximum_cords":i.maximum_cords,
                "maximum_routes":i.maximum_routes,"maximum_route_targets":i.maximum_route_targets,
                "maximum_host_calls":i.maximum_host_calls,"maximum_resources":i.maximum_resources,
                "maximum_sign_expectations":i.maximum_sign_expectations,
                "maximum_configuration_entries":i.maximum_configuration_entries,
                "maximum_ports_per_node":i.maximum_ports_per_node,
                "maximum_remote_endpoints":i.maximum_remote_endpoints,
                "maximum_cord_value_slots":i.maximum_cord_value_slots,
                "maximum_cord_value_bytes":i.maximum_cord_value_bytes,
                "maximum_sign_items":i.maximum_sign_items,"maximum_sign_bytes":i.maximum_sign_bytes
            },
            "maximum_program_bytes":self.computations.maximum_program_bytes,
            "maximum_total_program_bytes":self.computations.maximum_total_program_bytes,
            "assigned_encoded_bytes":self.maxima.encoded_bytes,
            "assigned_runtime_state_bytes":self.maxima.runtime_state_bytes,
            "assigned_counts":self.maxima.counts,
            "maximum_fore_ports":self.maximum_fore_ports,
            "maximum_document_bytes":maximum_document_bytes
        })
    }
}
