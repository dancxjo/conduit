//! Exact preparation of the shared five-Gear fan-out Tour Form.

use alloc::collections::BTreeMap;
use conduit_core::{ActivePlayIdentity, BaseImplementationId, Plan, PlanId, bind_active_play};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};

use crate::{
    execution_region::{seal_execution_region, validate_execution_region},
    identity::BootIdentities,
    offer::HostOffer,
    ordinary_plan::PreparationError,
    tour_morse_kernel::TourMorseKernel,
};

const FORM_NAME: &str = "branch-a-cord";
const LITERAL: &str = "sos";
const CORD_ITEMS: u16 = 1;
const CORD_BYTES: u32 = conduit_text::MAX_TEXT_BYTES;

pub struct PreparedTourMorsePlay {
    pub kernel: TourMorseKernel,
    pub scratch: crate::tour_morse_play::MorseScratch,
    pub plan: Plan,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
    pub plan_id: PlanId,
    pub active_play: ActivePlayIdentity,
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedTourMorsePlay, PreparationError> {
    let source =
        conduit_tour_model::tour_stage_source(0, 2).map_err(|_| PreparationError::FormRejected)?;
    let form = crate::ordinary_form::checked_expanded_text_form_named(&source, FORM_NAME)?;
    let advertisement = crate::ordinary_plan::advertisement(identities, offer, build_id)?;
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&form, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: CORD_ITEMS,
            connection_byte_capacity: CORD_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    let plan = seal_execution_region(plan, &advertisement, offer)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    validate_execution_region(fragment, &advertisement, offer)?;
    if fragment.placements.len() != 5 || fragment.connections.len() != 4 {
        return Err(PreparationError::PlanRejected);
    }
    let lowered = lower_plan_fragment(fragment).map_err(|_| PreparationError::LoweringRejected)?;
    if lowered.cord_value_slots > 4 || lowered.cord_value_bytes > CORD_BYTES * 4 {
        return Err(PreparationError::PlanRejected);
    }
    let kernel = TourMorseKernel::prepare(fragment, &lowered, LITERAL)
        .map_err(|_| PreparationError::KernelRejected)?;
    let scratch = crate::tour_morse_play::MorseScratch::prepared()
        .map_err(|_| PreparationError::KernelRejected)?;
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    Ok(PreparedTourMorsePlay {
        kernel,
        scratch,
        source_document_id: plan.source_document_id.clone(),
        checked_form_id: plan.checked_form_id.clone(),
        expanded_form_id: plan.expanded_form_id.clone(),
        plan_id: plan.plan_id.clone(),
        active_play,
        plan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offer::CpuFeatures;

    #[test]
    fn fanout_form_prepares_exact_native_morse_plan_and_kernel() {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        let prepared = prepare(&identities, &offer, "build").unwrap();
        assert!(conduit_core::verify_plan(&prepared.plan));
        assert_eq!(prepared.plan.fragments[0].placements.len(), 5);
        assert_eq!(prepared.plan.fragments[0].connections.len(), 4);
        assert_eq!(prepared.active_play.plan_id, prepared.plan_id);
    }
}
