//! Exact native Plan preparation for the shared standing timer Tour Form.

use alloc::collections::BTreeMap;
use conduit_core::{ActivePlayIdentity, BaseImplementationId, Plan, bind_active_play};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};

use crate::{
    execution_region::{seal_execution_region, validate_execution_region},
    identity::BootIdentities,
    offer::HostOffer,
    ordinary_plan::PreparationError,
};

const FORM_NAME: &str = "count-over-time";
const CORD_BYTES: u32 = 64;

pub struct PreparedTourTimerPlan {
    pub plan: Plan,
    pub active_play: ActivePlayIdentity,
    pub planned_sign_items: u16,
    pub planned_sign_bytes: u32,
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedTourTimerPlan, PreparationError> {
    let source =
        conduit_tour_model::tour_stage_source(2, 0).map_err(|_| PreparationError::FormRejected)?;
    let form = crate::ordinary_form::checked_expanded_tour_timer_form(&source, FORM_NAME)?;
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
            connection_item_capacity: 1,
            connection_byte_capacity: CORD_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    let plan = seal_execution_region(plan, &advertisement, offer)?;
    if !conduit_core::verify_plan(&plan)
        || plan.fragments.len() != 1
        || plan.fragments[0].placements.len() != 3
        || plan.fragments[0].connections.len() != 2
    {
        return Err(PreparationError::PlanRejected);
    }
    validate_execution_region(&plan.fragments[0], &advertisement, offer)?;
    let lowered =
        lower_plan_fragment(&plan.fragments[0]).map_err(|_| PreparationError::LoweringRejected)?;
    if lowered.cord_value_slots > 2
        || lowered.cord_value_bytes > CORD_BYTES * 2
        || lowered.sign_items > offer.sign_item_capacity
    {
        return Err(PreparationError::PlanRejected);
    }
    let active_play = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        0,
    );
    Ok(PreparedTourTimerPlan {
        plan,
        active_play,
        planned_sign_items: lowered.sign_items,
        planned_sign_bytes: lowered.sign_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offer::CpuFeatures;

    #[test]
    fn shared_standing_timer_form_plans_exact_native_implementations() {
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
        assert_eq!(prepared.active_play.plan_id, prepared.plan.plan_id);
        let implementations = prepared.plan.fragments[0]
            .placements
            .iter()
            .map(|placement| placement.implementation_id.as_str())
            .collect::<alloc::vec::Vec<_>>();
        for implementation in [
            crate::offer::TIME_EVERY_IMPLEMENTATION,
            crate::offer::STATE_COUNT_IMPLEMENTATION,
            crate::offer::COUNT_PRESENTATION_IMPLEMENTATION,
        ] {
            assert!(implementations.contains(&implementation));
        }
        assert!(!offer.capabilities[12].output.unwrap().closes);
        assert!(!offer.capabilities[13].input.unwrap().closes);
    }
}
