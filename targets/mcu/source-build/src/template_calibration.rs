//! Concrete checked-workset calibration from independently supplied equipment
//! descriptions. It never calls a runtime Ready binder or synthesizes grants.
use crate::{template, workset};
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_host_make as h;
use conduit_planner as p;
use std::{collections::BTreeMap, fmt::Write, path::Path};

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
pub struct Entry {
    pub name: String,
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
        entries.push(Entry {
            name: source.name.clone(),
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
    /// Static topology and definitions only; emitting these never installs a
    /// provider, resource grant, membership receipt or execution permission.
    pub fn emit_to(&self, out: &Path, maximum_generated_bytes: usize) -> Result<(), String> {
        let mut modules = Vec::new();
        let mut total = 0usize;
        for (index, entry) in self.entries.iter().enumerate() {
            let mut text = format!("pub const ENTRY:&str={:?};\npub const COUNTS:[u8;conduit_core::ASSIGNED_PLAN_COUNT_KINDS]={:?};\npub const SOURCE_TO_KERNEL:&[u16]=&{:?};\npub const RECORDS:&[conduit_core::AssignedRecordRequirement]=&[\n", entry.name, entry.layout.counts, entry.source_to_kernel);
            // Entry identity comes from the actual checked expansion, never
            // from a received projection or the template's placeholder Boot.
            let identities = format!("pub const SOURCE_DOCUMENT_ID:&str={:?};\npub const CHECKED_PLOT_ID:&str={:?};\npub const EXPANDED_PLOT_ID:&str={:?};\n",entry.source_document_id.as_str(),entry.checked_plot_id.as_str(),entry.expanded_plot_id.as_str());
            text.insert_str(0, &identities);
            for record in &entry.layout.records {
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
                "pub fn fore()->[conduit_core::AssignedForeRequirement;{}]{{[",
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
            text.push_str("]}\npub const OTHER_OFFER_IMAGES:&[&[u8]]=&[\n");
            for image in &entry.offer_images {
                writeln!(text, "&{:?},", image).unwrap();
            }
            text.push_str("];\n");
            total = total
                .checked_add(text.len())
                .ok_or("calibration generated length overflow")?;
            if total > maximum_generated_bytes {
                return Err("calibration exceeds selected generated-byte bound".into());
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
