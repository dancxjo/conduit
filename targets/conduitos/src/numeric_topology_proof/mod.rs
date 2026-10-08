//! Explicit synthetic numerical topology proof preparation.
//! The guest rechecks Source and plans for its current Host/Boot. Reference
//! transport never supplies current-Boot authority or a pretrained model.
mod catalog;
mod driver;
pub mod execution;
mod factories;
mod recipe;
pub mod resources;
pub mod storage;
pub mod embedded {
    include!(concat!(env!("OUT_DIR"), "/numeric_fixture.rs"));
}
use alloc::{format, string::String};
use conduit_core::*;
use conduit_plot::*;

pub const PROFILE: &str = "conduitos/synthetic-numeric-topology-preparation@1";
pub const MAXIMUM_NODES: usize = 1024;
pub const MAXIMUM_CORDS: usize = 2048;
/// Reference resource/ref contracts are broad; actual immutable ingress is separately limited to 16 KiB.
pub const MAXIMUM_BOUNDARY_CONTRACT_BYTES: u32 = 80 * 1024;
pub const ARENA_BYTES: usize = 256 * 1024 * 1024;
pub const PREPARATION_STACK_BYTES: usize = 64 * 1024 * 1024;

pub struct Materials<'a> {
    pub source: &'a [u8],
    pub reference_image: &'a [u8],
    pub native_definition: &'a str,
    pub recipe: &'a [u8],
}
pub struct PreparedTopology<'a> {
    materials: Materials<'a>,
    recipe: recipe::Recipe,
    context: catalog::Context,
    plan: Plan,
}
impl<'a> PreparedTopology<'a> {
    pub fn prepare(materials: Materials<'a>, host: HostId, boot: BootId) -> Result<Self, String> {
        if host.as_str().is_empty() || boot.as_str().is_empty() {
            return Err("numeric proof current Boot identity".into());
        }
        let reference = crate::local_plan_image::DecodedLocalPlanImage::decode(
            materials.reference_image,
            materials.source,
            crate::local_plan_image::LocalPlanImageBounds {
                image_bytes: crate::local_plan_image::MAXIMUM_IMAGE_BYTES,
                source_bytes: crate::local_plan_image::MAXIMUM_SOURCE_BYTES,
                placements: MAXIMUM_NODES,
                cords: MAXIMUM_CORDS,
            },
        )
        .map_err(|e| format!("reference image: {e:?}"))?;
        let recipe = recipe::Recipe::decode(materials.recipe)?;
        if recipe.source_document_id != reference.reference_plan().source_document_id
            || recipe.reference_plan_id != reference.reference_plan().plan_id
        {
            return Err("recipe and retained reference differ".into());
        }
        let reference_fragment = &reference.reference_plan().fragments[0];
        for fixture in &recipe.reference_fixture_placements {
            if !reference_fragment.placements.iter().any(|p| p == fixture) {
                return Err("foreign reference boundary placement".into());
            }
        }
        let expected = reference_fragment
            .placements
            .iter()
            .filter(|p| p.execution_profile_id.as_str() == "synthetic-epoch-driver@1")
            .count();
        if expected != recipe.reference_fixture_placements.len() {
            return Err("incomplete reference boundary coverage".into());
        }
        let mut context = catalog::Context::prepare(&recipe, materials.native_definition)?;
        let text = core::str::from_utf8(materials.source)
            .map_err(|_| String::from("numeric Source UTF8"))?;
        let checked = check_syntax_document(&parse_syntax_document(text), &context.startup)
            .map_err(|e| format!("numeric Source check: {e:?}"))?;
        let expanded =
            expand_canonical_plot_for_authoring(&checked, &recipe.entry_plot, &context.profiles)
                .map_err(|e| format!("numeric Source expansion: {e:?}"))?;
        if !expanded.input_bindings.is_empty()
            || !expanded.output_bindings.is_empty()
            || expanded.expanded.source_document_id != reference.reference_plan().source_document_id
            || expanded.expanded.checked_plot_id != reference.reference_plan().checked_plot_id
            || expanded.expanded.expanded_plot_id != reference.reference_plan().expanded_plot_id
            || expanded.expanded.gears.len() > MAXIMUM_NODES
            || expanded.expanded.connections.len() > MAXIMUM_CORDS
        {
            return Err("numeric checked/expanded reference correspondence".into());
        }
        for gear in &expanded.expanded.gears {
            let selected = if context.offers.iter().any(|o| o.kind_id == gear.kind_id) {
                None
            } else if gear.kind_id.as_str().starts_with("numeric/") {
                Some(catalog::numeric_offer(gear.kind_id.as_str())?)
            } else if matches!(
                gear.kind_contract_revision.as_str(),
                PURE_EXPRESSION_REVISION | PURE_FILTER_REVISION
            ) {
                let [entry] = gear.configuration.as_slice() else {
                    return Err("numeric expression configuration".into());
                };
                let ConfigurationValue::Text(encoded) = &entry.value else {
                    return Err("numeric expression program".into());
                };
                let program = PortableExpressionProgram::from_canonical_hex(encoded)
                    .map_err(|e| format!("{e:?}"))?;
                Some(
                    if gear.kind_contract_revision.as_str() == PURE_FILTER_REVISION {
                        crate::pure_filter::offer(&program, gear.inputs[0].temporal)
                            .map_err(|e| format!("{e:?}"))?
                    } else {
                        crate::expression_host_call::offer(&program, gear.outputs[0].temporal)
                            .map_err(|e| format!("{e:?}"))?
                    },
                )
            } else {
                None
            };
            if let Some(offer) = selected
                && !context
                    .offers
                    .iter()
                    .any(|o| o.capability_id == offer.capability_id)
            {
                context.offers.push(offer);
            }
        }
        let advertisement = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: host,
            boot_id: boot,
            offer_generation: OfferGeneration(1),
            profile: PROFILE.into(),
            bases: alloc::vec![],
            resources: alloc::vec![],
            planner_capabilities: alloc::vec![],
            capabilities: context.offers.clone(),
        };
        let placements = conduit_planner::default_expanded_placements(
            &expanded.expanded,
            core::slice::from_ref(&advertisement),
        )
        .map_err(|e| format!("{e:?}"))?;
        let plan = conduit_planner::plan_expanded_canonical(
            &expanded.expanded,
            &[advertisement],
            &placements,
            &["conduit.base/local@1".into()],
        )
        .map_err(|e| format!("{e:?}"))?;
        if !verify_plan(&plan) {
            return Err("numeric guest Plan seal".into());
        }
        Ok(Self {
            materials,
            recipe,
            context,
            plan,
        })
    }
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    pub fn retained_source_bytes(&self) -> usize {
        self.materials.source.len()
    }
    pub fn retained_material_extents(&self) -> [usize; 4] {
        [
            self.materials.source.len(),
            self.materials.reference_image.len(),
            self.materials.native_definition.len(),
            self.materials.recipe.len(),
        ]
    }
    pub fn admitted_profile_counts(&self) -> [usize; 4] {
        [
            self.context.native.len(),
            self.context.weakening.len(),
            self.context.guards.len(),
            self.context.pairs.len(),
        ]
    }
    pub fn scalar_profile_identity(&self) -> String {
        self.context.scalar.kind_identity(true)
    }
    pub fn recipe_counts(&self) -> [usize; 3] {
        [
            self.recipe.resources.len(),
            self.recipe.inputs.len(),
            self.recipe.reference_fixture_placements.len(),
        ]
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod execution_tests;
