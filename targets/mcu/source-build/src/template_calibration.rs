//! Concrete checked-workset calibration from independently supplied equipment
//! descriptions. It never calls a runtime Ready binder or synthesizes grants.
use crate::{template, workset};
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_host_make as h;
use conduit_planner as p;
use std::{collections::BTreeMap, fmt::Write, path::Path};

#[path = "template_continuation_manifest.rs"]
mod continuation_manifest;
#[path = "template_seal_emission.rs"]
mod seal_emission;

pub struct EntryLimits {
    pub entry: String,
    pub connections: BTreeMap<p::ConnectionEndpoints, p::ConnectionQueueLimits>,
    pub boundaries: BTreeMap<p::ForeBoundaryKey, p::ConnectionQueueLimits>,
}
pub struct Inputs<'a> {
    pub checked_host: &'a h::CheckedHostConfiguration,
    pub image: &'a h::HostImage,
    pub catalog: &'a h::MakeCatalog,
    /// Actual definition/resource/Base facts supplied by checked construction.
    /// A descriptive calibration Boot is permitted; no Ready publication occurs.
    pub host: &'a c::HostAdvertisement,
    /// Inert descriptions from independently checked construction policy.
    /// Placeholder Boot bindings are calibration inputs, never actual grants.
    /// They are consumed only by the structural compiler and are not emitted.
    pub authority_descriptions: &'a [c::AuthorityGrant],
    pub protected_resource_descriptions: &'a [c::ProtectedResourceGrant],
    pub resident: &'a [&'a e::PreparedResidentExpression],
    pub entries: &'a [EntryLimits],
    pub internal_queue: p::ConnectionQueueLimits,
    pub image_bounds: e::EmbeddedImageBounds,
    pub computations: e::EmbeddedComputationBounds,
    pub maxima: c::AssignedPlanMaxima,
    pub maximum_fore_ports: usize,
    pub maximum_offer_image_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskRole {
    Tee,
    Renderer,
    ReadOnly,
}
pub struct Entry {
    pub name: String,
    pub mask_roles: Option<[MaskRole; 3]>,
    pub append_status: Option<c::AssignedForeRequirement>,
    pub source_document_id: c::SourceDocumentId,
    pub checked_plot_id: c::CheckedPlotId,
    pub expanded_plot_id: c::ExpandedPlotId,
    pub layout: template::Layout,
    /// Exact selected definitions, not provider handles or current offers.
    pub other: Vec<c::CapabilityOffer>,
    pub source_to_kernel: Vec<u16>,
    pub offer_images: Vec<Vec<u8>>,
}
pub struct Calibration {
    pub entries: Vec<Entry>,
}

pub fn compile(workset: &workset::Workset, input: Inputs<'_>) -> Result<Calibration, String> {
    h::verify_configuration_image(input.checked_host, input.image, input.catalog)?;
    if input.entries.len() != workset.entries.len()
        || input.internal_queue.item_capacity == 0
        || input.internal_queue.byte_capacity == 0
        || input.maximum_offer_image_bytes == 0
    {
        return Err(
            "calibration requires one independently bounded selection per checked entry".into(),
        );
    }
    let mut entries = Vec::new();
    for (source, limits) in workset.entries.iter().zip(input.entries) {
        if source.name != limits.entry {
            return Err("calibration entry order differs from exact checked workset".into());
        }
        let placements =
            p::default_expanded_placements(&source.plot.expanded, std::slice::from_ref(input.host))
                .map_err(debug)?;
        let mut other = Vec::new();
        for placement in placements.by_gear.values() {
            let offer = input
                .host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .ok_or("calibration selected absent capability definition")?;
            if !input
                .resident
                .iter()
                .any(|image| image.offer_description() == offer)
                && !other
                    .iter()
                    .any(|prior: &c::CapabilityOffer| prior.capability_id == offer.capability_id)
            {
                other.push(offer.clone());
            }
        }
        let others: Vec<_> = other.iter().collect();
        let empty = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let layout = template::compile(template::Inputs {
            plot: &source.plot,
            checked_source: Some(&workset.checked_source),
            host: input.host,
            placements: &placements,
            planning: p::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &line_candidates,
                connection_item_capacity: input.internal_queue.item_capacity,
                connection_byte_capacity: input.internal_queue.byte_capacity,
                authority_grants: input.authority_descriptions,
                protected_resource_grants: input.protected_resource_descriptions,
                line_offers: &[],
            },
            connections: &limits.connections,
            boundaries: &limits.boundaries,
            resident: input.resident,
            other: &others,
            image: input.image_bounds,
            computations: input.computations,
            maxima: input.maxima,
            maximum_fore_ports: input.maximum_fore_ports,
        })?;
        let source_to_kernel = source
            .plot
            .expanded
            .gears
            .iter()
            .map(|gear| {
                let mut selected = layout.nodes.iter().filter(|(_, id)| id == &gear.gear_id);
                let node = selected
                    .next()
                    .ok_or("calibration lost checked Source node")?
                    .0;
                if selected.next().is_some() {
                    return Err("calibration duplicated checked Source node".into());
                }
                Ok(node)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let offer_images = other
            .iter()
            .map(|offer| {
                e::generate_capability_offer_image(offer, input.maximum_offer_image_bytes)
                    .map_err(debug)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mask_roles = selected_mask_roles(&source.plot, &layout)?;
        let append_status = selected_append_status(&source.plot, &layout)?;
        entries.push(Entry {
            name: source.name.clone(),
            mask_roles,
            append_status,
            source_document_id: source.plot.expanded.source_document_id.clone(),
            checked_plot_id: source.plot.expanded.checked_plot_id.clone(),
            expanded_plot_id: source.plot.expanded.expanded_plot_id.clone(),
            layout,
            other,
            source_to_kernel,
            offer_images,
        });
    }
    Ok(Calibration { entries })
}

impl Calibration {
    /// Install the proven Source/kernel permutation into generated software
    /// factories before emitting firmware. Stage every entry before mutation.
    pub fn apply_software_mapping(
        &self,
        workset: &mut workset::Workset,
        mask: Option<workset::MaskBounds>,
        maximum_image_bytes: usize,
        maximum_generated_bytes: usize,
    ) -> Result<(), String> {
        if self.entries.len() != workset.entries.len() {
            return Err("calibration differs from complete checked workset".into());
        }
        let mut staged = Vec::new();
        for (calibrated, source) in self.entries.iter().zip(&workset.entries) {
            if calibrated.name != source.name
                || calibrated.source_document_id != source.plot.expanded.source_document_id
                || calibrated.checked_plot_id != source.plot.expanded.checked_plot_id
                || calibrated.expanded_plot_id != source.plot.expanded.expanded_plot_id
            {
                return Err("calibration lost exact checked entry identity".into());
            }
            let mut rust = crate::software::emit_rust_for_layout_with_types(
                &source.plot.expanded,
                Some(&workset.checked_source),
                &calibrated.source_to_kernel,
                maximum_image_bytes,
                maximum_generated_bytes,
            )?;
            workset::emit_mask_routing_images(&source.plot.expanded, &mut rust, mask)?;
            if rust.len() > maximum_generated_bytes {
                return Err("calibrated software exceeds selected generated-byte bound".into());
            }
            staged.push(rust);
        }
        for (source, rust) in workset.entries.iter_mut().zip(staged) {
            source.rust = rust;
        }
        workset.calibrated = true;
        Ok(())
    }

    /// Static topology and definitions only; emitting these never installs a
    /// provider, resource grant, membership receipt or execution permission.
    pub fn emit_to(&self, out: &Path, maximum_generated_bytes: usize) -> Result<(), String> {
        let mut modules = Vec::new();
        let mut total = 0usize;
        for (index, entry) in self.entries.iter().enumerate() {
            let mut text = format!("pub const ENTRY:&str={:?};\npub const COUNTS:[u8;conduit_core::ASSIGNED_PLAN_COUNT_KINDS]={:?};\npub const SOURCE_TO_KERNEL:&[u16]=&{:?};\npub const RECORDS:&[conduit_core::AssignedRecordRequirement]=&[\n", entry.name, entry.layout.counts, entry.source_to_kernel);
            let status = match entry.append_status {
                None => "None".to_owned(),
                Some(pin) => format!("Some(conduit_core::AssignedForeRequirement::new({},{},conduit_core::AssignedForeDirection::Output,conduit_core::AssignedIdentity({:?}),{:?}))",pin.endpoint(),pin.cord(),&pin.payload()[5..21],&pin.payload()[21..]),
            };
            text.insert_str(0, &format!("pub fn append_status_fore()->Option<conduit_core::AssignedForeRequirement>{{{status}}}\n"));
            text.insert_str(0, &entry.layout.continuation.emit());
            let continuation = format!(
                "// Inert checked construction; not a sealed current-Boot Plan or execution grant.\npub const PROJECTION_TEMPLATE:&[u8]=&{:?};\npub const PROJECTION_TEMPLATE_DIGEST:[u8;32]={:?};\n",
                entry.layout.projection_template,
                entry.layout.projection_template_digest,
            );
            text.insert_str(0, &continuation);
            text.insert_str(0, &format!("pub const FRAGMENT_RECIPE_DIGEST:[u8;32]={:?};\npub const FRAGMENT_PLACEMENT_IDS:&[&str]=&{:?};\n", c::fragment_seal_recipe_identity(entry.layout.fragment_seal_recipe.pieces()), entry.layout.fragment_seal_placement_ids));
            let seal_rust = seal_emission::emit(&entry.layout.fragment_seal_recipe);
            text.insert_str(0, &seal_rust);
            text.insert_str(
                0,
                &format!(
                    "pub const FRAGMENT_SEAL_TO_KERNEL:&[u16]=&{:?};\n",
                    entry.layout.fragment_seal_to_kernel
                ),
            );
            let geometry = format!(
                "pub const CORD_VALUE_SLOT_CAPACITIES:&[u32]=&{:?};\npub const STEP_VALUE_SLOT_CAPACITIES:&[u32]=&{:?};\npub const HOST_CALL_VALUE_SLOT_CAPACITIES:&[u32]=&{:?};\n",
                entry.layout.cord_value_slot_capacities,
                entry.layout.step_value_slot_capacities,
                entry.layout.host_call_value_slot_capacities,
            );
            text.insert_str(0, &geometry);
            if let Some(profiles) = &entry.layout.value_profiles_rust {
                text.insert_str(0, profiles);
            }
            let gears: Vec<_> = entry
                .layout
                .nodes
                .iter()
                .map(|(node, gear)| (*node, gear.as_str()))
                .collect();
            text.insert_str(
                0,
                &format!("pub static GEAR_IDS:&[(u16,&str)]=&{gears:?};\n"),
            );
            let storage=format!("pub const PORTS_PER_NODE:usize={};\npub const SIGN_ITEMS:usize={};\npub const SIGN_BYTES:u32={};\n",entry.layout.ports_per_node,entry.layout.sign_items,entry.layout.sign_bytes);
            text.insert_str(0, &storage);
            text.insert_str(0, &format!("pub const HOST_CALL_SLOTS:usize={};\npub const PENDING_HOST_CALL_SLOTS:usize={};\n", entry.layout.host_call_slots, entry.layout.pending_host_call_slots));
            text.insert_str(0, &entry.layout.terminal_transductions_rust);
            text.insert_str(
                0,
                &format!(
                    "pub const RECORD_SERVICE_RESOURCE:Option<u16>={:?};\n",
                    entry.layout.record_service_resource
                ),
            );
            let mut profile=format!("pub const MAXIMA:conduit_core::AssignedPlanMaxima=conduit_core::AssignedPlanMaxima{{encoded_bytes:{},runtime_state_bytes:{},counts:{:?}}};\npub const HOST_CALL_CONTRACTS:&[conduit_core::AssignedIdentity]=&[",entry.layout.maxima.encoded_bytes,entry.layout.maxima.runtime_state_bytes,entry.layout.maxima.counts);
            for identity in &entry.layout.host_call_contracts {
                write!(profile, "conduit_core::AssignedIdentity({:?}),", identity.0).unwrap();
            }
            write!(profile,"];\npub const RESOURCE_ORDINALS:&[u16]=&{:?};\npub const COMPUTATIONS:&[conduit_core::AssignedComputationRequirement]=&[",entry.layout.resource_ordinals).unwrap();
            for pin in &entry.layout.computations {
                write!(profile,"conduit_core::AssignedComputationRequirement{{node:{},key:conduit_core::AssignedIdentity({:?}),text_bytes:{},text_digest:{:?}}},",pin.node,pin.key.0,pin.text_bytes,pin.text_digest).unwrap();
            }
            write!(profile,"];\npub const COMPUTATION_MAXIMUM_TEXT_BYTES:u32={};\npub const COMPUTATION_MAXIMUM_TOTAL_TEXT_BYTES:u32={};\n",entry.layout.maximum_computation_text_bytes,entry.layout.maximum_total_computation_text_bytes).unwrap();
            text.insert_str(0, &profile);
            match entry.mask_roles {
                None => text.insert_str(0, "pub const MASK_ROLES:Option<[conduit_seven_segment_mask::native_kernel::MaskRole;3]>=None;\n"),
                Some(roles) => {
                    let mut declaration=String::from("pub const MASK_ROLES:Option<[conduit_seven_segment_mask::native_kernel::MaskRole;3]>=Some([");
                    for role in roles {
                        let name=match role {MaskRole::Tee=>"Tee",MaskRole::Renderer=>"Renderer",MaskRole::ReadOnly=>"ReadOnly"};
                        write!(declaration,"conduit_seven_segment_mask::native_kernel::MaskRole::{name},").unwrap();
                    }
                    declaration.push_str("]);\n");text.insert_str(0,&declaration);
                }
            }
            // Entry identity comes from the actual checked expansion, never
            // from a received projection or the template's placeholder Boot.
            let identities = format!("pub const SOURCE_DOCUMENT_ID:&str={:?};\npub const CHECKED_PLOT_ID:&str={:?};\npub const EXPANDED_PLOT_ID:&str={:?};\n",entry.source_document_id.as_str(),entry.checked_plot_id.as_str(),entry.expanded_plot_id.as_str());
            text.insert_str(0, "pub const PLOT_PLAN_SEAL:conduit_core::PlainPlotPlanSeal<'static>=conduit_core::PlainPlotPlanSeal{source_document_id:SOURCE_DOCUMENT_ID,checked_plot_id:CHECKED_PLOT_ID,expanded_plot_id:EXPANDED_PLOT_ID};\n");
            text.insert_str(0, &identities);
            for record in &entry.layout.records {
                writeln!(
                    text,
                    "conduit_core::AssignedRecordRequirement{{tag:{},payload_digest:{:?}}},",
                    record.tag, record.payload_digest
                )
                .unwrap();
            }
            text.push_str(
                "];\npub const CONFIGURATIONS:&[conduit_core::AssignedRecordRequirement]=&[\n",
            );
            for record in &entry.layout.configurations {
                writeln!(
                    text,
                    "conduit_core::AssignedRecordRequirement{{tag:{},payload_digest:{:?}}},",
                    record.tag, record.payload_digest
                )
                .unwrap();
            }
            text.push_str("];\n");
            writeln!(
                text,
                "pub const FORE_PORTS:usize={};",
                entry.layout.fore.len()
            )
            .unwrap();
            writeln!(
                text,
                "pub static FORE:[conduit_core::AssignedForeRequirement;{}]=[",
                entry.layout.fore.len()
            )
            .unwrap();
            for fore in &entry.layout.fore {
                let payload = fore.payload();
                let front: [u8; 16] = payload[5..21].try_into().unwrap();
                let abi: [u8; 32] = payload[21..].try_into().unwrap();
                let direction = match fore.direction() {
                    c::AssignedForeDirection::Input => "Input",
                    c::AssignedForeDirection::Output => "Output",
                };
                writeln!(text,"conduit_core::AssignedForeRequirement::new({},{},conduit_core::AssignedForeDirection::{},conduit_core::AssignedIdentity({:?}),{:?}),",fore.endpoint(),fore.cord(),direction,front,abi).unwrap();
            }
            writeln!(
                text,
                "];\npub fn fore()->[conduit_core::AssignedForeRequirement;{}]{{FORE}}",
                entry.layout.fore.len()
            )
            .unwrap();
            text.push_str("pub const OTHER_OFFER_IMAGES:&[&[u8]]=&[\n");
            for image in &entry.offer_images {
                writeln!(text, "&{:?},", image).unwrap();
            }
            text.push_str("];\n");
            total = total
                .checked_add(text.len())
                .ok_or("calibration generated length overflow")?;
            if total > maximum_generated_bytes {
                return Err(format!(
                    "calibration exceeds selected generated-byte bound: required {total}, maximum {maximum_generated_bytes}; entry {:?}: seal {}, projection {}, value profiles {}, terminal transductions {}, offer images {}",
                    entry.name, seal_rust.len(), continuation.len(),
                    entry.layout.value_profiles_rust.as_ref().map_or(0, String::len),
                    entry.layout.terminal_transductions_rust.len(),
                    entry.offer_images.iter().map(Vec::len).sum::<usize>(),
                ));
            }
            modules.push((format!("workset_{index}_layout.rs"), text));
        }
        // Complete bounds validation precedes any output-file effect.
        for (file, text) in modules {
            std::fs::write(out.join(file), text).map_err(debug)?;
        }
        Ok(())
    }
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("workset calibration: {error:?}")
}

/// The finite three-Back Mask profile is chosen by an actual renderer in the
/// checked entry. Resolve roles from the lowered mapping, never Source indices.
fn selected_mask_roles(
    plot: &conduit_plot::ExpandedAuthoringPlot,
    layout: &template::Layout,
) -> Result<Option<[MaskRole; 3]>, String> {
    if !plot
        .expanded
        .gears
        .iter()
        .any(|g| g.kind_id.as_str() == "face/renderer")
    {
        return Ok(None);
    }
    if layout.nodes.len() != 3 {
        return Err("selected Mask profile requires exactly three lowered Backs".into());
    }
    let mut roles = [None; 3];
    for (node, id) in &layout.nodes {
        let gear = plot
            .expanded
            .gears
            .iter()
            .find(|g| &g.gear_id == id)
            .ok_or("Mask role lost checked Source Gear")?;
        let role = match gear.kind_id.as_str() {
            "face/tee" => MaskRole::Tee,
            "face/renderer" => MaskRole::Renderer,
            "face/interaction" => MaskRole::ReadOnly,
            _ => return Err("selected three-Back Mask has an unsupported checked Gear".into()),
        };
        let slot = roles
            .get_mut(usize::from(*node))
            .ok_or("Mask node ordinal outside selected profile")?;
        if slot.replace(role).is_some() {
            return Err("duplicate lowered Mask node ordinal".into());
        }
    }
    let roles = roles.map(|r| r.ok_or("missing lowered Mask role"));
    let [tee, renderer, interaction] = roles;
    let roles = [tee?, renderer?, interaction?];
    for role in [MaskRole::Tee, MaskRole::Renderer, MaskRole::ReadOnly] {
        if roles.iter().filter(|r| **r == role).count() != 1 {
            return Err("ambiguous checked Mask roles".into());
        }
    }
    Ok(Some(roles))
}

fn selected_append_status(
    plot: &conduit_plot::ExpandedAuthoringPlot,
    layout: &template::Layout,
) -> Result<Option<c::AssignedForeRequirement>, String> {
    let Some(port) = plot
        .front
        .outputs()
        .iter()
        .find(|port| port.port_id.as_str() == "append_status")
    else {
        return Ok(None);
    };
    let kind = conduit_semantic_catalog::resource_append_semantic_contract();
    let expected = &kind.outputs[0];
    let contract = plot
        .front
        .value_contract(&c::FrontValueLocation::Output(port.port_id.clone()));
    let expected_contract = kind.checked_front();
    if port.value_kind != expected.value_kind
        || port.temporal != expected.temporal
        || port.direction != c::PortDirection::Output
        || port.abnormal_kind.is_some()
        || contract
            != expected_contract
                .value_contract(&c::FrontValueLocation::Output(expected.port_id.clone()))
        || plot
            .output_bindings
            .iter()
            .filter(|binding| {
                binding.front_port_id == port.port_id
                    && binding.track == c::ConnectionTrack::Payload
            })
            .count()
            != 1
    {
        return Err(
            "append_status must expose the exact finite canonical append outcome Flow".into(),
        );
    }
    let identity = c::AssignedIdentity::from_text("append_status");
    let mut pins = layout.fore.iter().filter(|pin| {
        pin.direction() == c::AssignedForeDirection::Output && pin.payload()[5..21] == identity.0
    });
    let pin = pins
        .next()
        .copied()
        .ok_or("append_status has no calibrated Fore ABI")?;
    if pins.next().is_some() {
        return Err("duplicate append_status calibrated Fore".into());
    }
    Ok(Some(pin))
}
