//! Source-first firmware construction, with no Host, native readiness or Plan.
use crate::SoftwareRecipeRefusal as E;
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::{
        assigned::{
            ResidentComputation, ResidentRoles, ResidentSoftwareOffer, ResidentSoftwareSpec,
            ResidentSourceIdentity,
        },
        software::compile_offer_templates,
    },
    construction::ApplianceSource,
};
use std::fmt::{self, Write};
#[path = "source_recipe/records.rs"]
mod records;

pub struct NativeSoftwareSourceRecipe {
    identity: c::PlotIdentity,
    source: ResidentSourceIdentity,
    roles: ResidentRoles,
    cadence: conduit_time::TickConfiguration,
    computations: Vec<c::AssignedComputationRequirement>,
    texts: Vec<(u16, c::AssignedIdentity, String, Vec<u8>)>,
    configuration_records: Vec<Vec<u8>>,
    offers: Vec<(u16, Vec<u8>)>,
    largest: u16,
    total: u32,
}
impl NativeSoftwareSourceRecipe {
    /// Compile only checked meaning and the portable software implementation.
    /// Actual native providers and all realization/Body authority remain absent.
    pub fn prepare(source: &ApplianceSource) -> Result<Self, E> {
        let cadence = source.checked_cadence().map_err(|_| E::Source)?;
        let gears = &source.expanded.gears;
        if gears.len() != conduit_environment_runtime::appliance::assigned::NODES {
            return Err(E::Roles);
        }
        let node = |index| u16::try_from(index).map_err(|_| E::Capacity);
        let roles = ResidentRoles::from_nodes(
            gears
                .iter()
                .enumerate()
                .map(|(i, g)| (i as u16, g.kind_id.as_str())),
        )
        .map_err(|_| E::Roles)?;
        let mut texts = Vec::new();
        let mut computations = Vec::new();
        for (index, gear) in gears.iter().enumerate() {
            for entry in &gear.configuration {
                if !matches!(entry.key.as_str(), "program" | "selector") {
                    continue;
                }
                let c::ConfigurationValue::Text(text) = &entry.value else {
                    return Err(E::Source);
                };
                let requirement =
                    c::AssignedComputationRequirement::from_text(node(index)?, &entry.key, text)
                        .map_err(|_| E::Source)?;
                let bytes = text
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| {
                        u8::from_str_radix(core::str::from_utf8(pair).map_err(|_| E::Source)?, 16)
                            .map_err(|_| E::Source)
                    })
                    .collect::<Result<Vec<_>, E>>()?;
                texts.push((requirement.node, requirement.key, text.clone(), bytes));
                computations.push(requirement);
            }
        }
        if computations.len() != conduit_environment_runtime::appliance::assigned::NODES - 8 {
            return Err(E::Roles);
        }
        let largest = computations
            .iter()
            .map(|c| c.text_bytes)
            .max()
            .ok_or(E::Roles)?;
        let total = computations
            .iter()
            .try_fold(0_u32, |sum, c| sum.checked_add(u32::from(c.text_bytes)))
            .ok_or(E::Capacity)?;
        let offers = compile_offer_templates(source)
            .map_err(|_| E::Offers)?
            .into_iter()
            .map(|(gear, offer)| {
                let index = gears
                    .iter()
                    .position(|g| g.gear_id == gear)
                    .ok_or(E::Roles)?;
                Ok((
                    node(index)?,
                    serde_json::to_vec(&offer).map_err(|_| E::Offers)?,
                ))
            })
            .collect::<Result<Vec<_>, E>>()?;
        let configuration_records = records::configuration_payloads(&texts);
        Ok(Self {
            identity: c::PlotIdentity {
                source_document_id: source.expanded.source_document_id.clone(),
                checked_plot_id: source.expanded.checked_plot_id.clone(),
                expanded_plot_id: source.expanded.expanded_plot_id.clone(),
            },
            source: ResidentSourceIdentity::from_expanded(&source.expanded.expanded_plot_id),
            roles,
            cadence,
            computations,
            texts,
            configuration_records,
            offers,
            largest,
            total,
        })
    }
    pub fn spec(&self) -> ResidentSoftwareSpec<'_> {
        ResidentSoftwareSpec {
            cadence: self.cadence.clone(),
            source: self.source,
            roles: self.roles,
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: self.largest,
                maximum_total_text_bytes: self.total,
                requirements: &self.computations,
            },
        }
    }
    pub fn cadence(&self) -> &conduit_time::TickConfiguration {
        &self.cadence
    }
    pub fn resident_computations(&self) -> impl ExactSizeIterator<Item = ResidentComputation<'_>> {
        self.texts
            .iter()
            .map(|(node, key, text, bytes)| ResidentComputation {
                node: *node,
                key: *key,
                text,
                canonical_bytes: Some(bytes),
            })
    }
    pub fn resident_offers(&self) -> Vec<ResidentSoftwareOffer<'_>> {
        self.offers
            .iter()
            .map(|(node, json)| {
                // SAFETY: exact metadata comes from the checked Source's ordinary
                // portable constructor above, paired with this same pinned recipe.
                unsafe { ResidentSoftwareOffer::from_make(*node, json) }
            })
            .collect()
    }
    pub(crate) fn offer_json(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.offers.iter().map(|(_, json)| json.as_slice())
    }
    /// Emit host-free static construction data; this initializes no software.
    pub fn write_source_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "// Make-checked Source construction; no current Host, Boot, Plan or authority."
        )?;
        writeln!(out, "use conduit_core as c;")?;
        writeln!(out, "pub fn source_identity() -> c::PlotIdentity {{ c::PlotIdentity {{ source_document_id: {:?}.into(), checked_plot_id: {:?}.into(), expanded_plot_id: {:?}.into() }} }}", self.identity.source_document_id.as_str(), self.identity.checked_plot_id.as_str(), self.identity.expanded_plot_id.as_str())?;
        writeln!(out,"use conduit_environment_runtime::appliance::assigned::{{ResidentComputation, ResidentRoles, ResidentSoftwareOffer, ResidentSoftwareSpec, ResidentSourceIdentity}};")?;
        crate::software_recipe::write_material(out, &self.computations, &self.texts, &self.offers)?;
        self.write_computation_records(out)?;
        writeln!(out,"pub fn spec() -> ResidentSoftwareSpec<'static> {{ ResidentSoftwareSpec {{ cadence:cadence(), source: ResidentSourceIdentity {{ expanded_plot_digest: {:?} }}, computations: c::AssignedComputationProfile {{ maximum_text_bytes: {}, maximum_total_text_bytes: {}, requirements: &COMPUTATIONS }},",self.source.expanded_plot_digest,self.largest,self.total)?;
        let r = self.roles;
        writeln!(out,"roles:ResidentRoles {{ pulse:conduit_kernel::NodeId({}),tick:conduit_kernel::NodeId({}),trigger:conduit_kernel::NodeId({}),observation:conduit_kernel::NodeId({}),publications:[",r.pulse.0,r.tick.0,r.trigger.0,r.observation.0)?;
        for n in r.publications {
            writeln!(out, "conduit_kernel::NodeId({}),", n.0)?;
        }
        writeln!(out, "] }} }} }}")?;
        writeln!(out,"pub fn cadence() -> conduit_time::TickConfiguration {{ conduit_time::TickConfiguration {{ count:{},period_ms:{} }} }}",self.cadence.count,self.cadence.period_ms)
    }
}
