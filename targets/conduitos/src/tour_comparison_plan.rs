//! Distinct direct and recursively expanded Plans for the shared comparison Tour Form.

use alloc::collections::BTreeMap;
use conduit_core::{ActivePlayIdentity, BaseImplementationId, Plan, bind_active_play};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_planner::{
    ConnectionQueueLimits, PlanningOptions, default_expanded_placements,
    plan_expanded_canonical_with_connection_limits,
};

use crate::{
    execution_region::{seal_execution_region, validate_execution_region},
    identity::BootIdentities,
    offer::HostOffer,
    ordinary_plan::PreparationError,
};

const FORM_NAME: &str = "same-morse-caller";
const CORD_BYTES: u32 = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32;

pub struct PreparedComparisonPlans {
    pub direct: Plan,
    pub recursive: Plan,
    pub direct_active: ActivePlayIdentity,
    pub recursive_active: ActivePlayIdentity,
    pub(crate) direct_kernel: crate::tour_comparison_kernel::ComparisonKernel,
    pub(crate) recursive_kernel: crate::tour_comparison_kernel::ComparisonKernel,
    pub(crate) scratch: crate::tour_comparison_play::ComparisonScratch,
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedComparisonPlans, PreparationError> {
    let source =
        conduit_tour_model::tour_stage_source(1, 0).map_err(|_| PreparationError::FormRejected)?;
    let direct = crate::ordinary_form::checked_expanded_text_form_named(&source, FORM_NAME)?;
    let recursive =
        crate::ordinary_form::checked_expanded_text_form_with_morse_backs(&source, FORM_NAME)?;
    if direct.expanded_form_id == recursive.expanded_form_id
        || direct.gears.len() != 3
        || recursive.gears.len() != 7
        || recursive.realization_backs.len() != 2
    {
        return Err(PreparationError::FormRejected);
    }
    let advertisement = crate::ordinary_plan::advertisement(identities, offer, build_id)?;
    let direct =
        plan(&direct, &advertisement, offer).map_err(|_| PreparationError::PlanRejected)?;
    let recursive = plan(&recursive, &advertisement, offer)?;
    let direct_lowered = lower_plan_fragment(&direct.fragments[0])
        .map_err(|_| PreparationError::LoweringRejected)?;
    let recursive_lowered = lower_plan_fragment(&recursive.fragments[0])
        .map_err(|_| PreparationError::LoweringRejected)?;
    let direct_kernel = crate::tour_comparison_kernel::ComparisonKernel::prepare_direct(
        &direct.fragments[0],
        &direct_lowered,
    )
    .map_err(|_| PreparationError::KernelRejected)?;
    let recursive_kernel = crate::tour_comparison_kernel::ComparisonKernel::prepare_recursive(
        &recursive.fragments[0],
        &recursive_lowered,
    )
    .map_err(|_| PreparationError::KernelRejected)?;
    let direct_active = bind_active_play(
        &direct.plan_id,
        &direct.fragments[0].host_id,
        &direct.fragments[0].boot_id,
        0,
    );
    let recursive_active = bind_active_play(
        &recursive.plan_id,
        &recursive.fragments[0].host_id,
        &recursive.fragments[0].boot_id,
        0,
    );
    Ok(PreparedComparisonPlans {
        direct,
        recursive,
        direct_active,
        recursive_active,
        direct_kernel,
        recursive_kernel,
        scratch: crate::tour_comparison_play::ComparisonScratch::prepared(),
    })
}

fn plan(
    form: &conduit_form::ExpandedCanonicalForm,
    advertisement: &conduit_core::HostAdvertisement,
    offer: &HostOffer<'_>,
) -> Result<Plan, PreparationError> {
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(form, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    let limits = form
        .connections
        .iter()
        .map(|connection| {
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: value_bound(connection.value_kind.as_str()),
                },
            )
        })
        .collect();
    let plan = plan_expanded_canonical_with_connection_limits(
        form,
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
        &limits,
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    let plan = seal_execution_region(plan, advertisement, offer)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PreparationError::PlanRejected);
    }
    validate_execution_region(&plan.fragments[0], advertisement, offer)?;
    Ok(plan)
}

fn value_bound(kind: &str) -> u32 {
    match kind {
        conduit_text::TEXT_VALUE_KIND => conduit_text::MAX_TEXT_BYTES,
        conduit_text::MORSE_CHARACTERS_VALUE_KIND => {
            conduit_text::MAXIMUM_MORSE_CHARACTERS_BYTES as u32
        }
        conduit_text::MORSE_SYMBOL_GROUPS_VALUE_KIND => {
            conduit_text::MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES as u32
        }
        conduit_text::MORSE_GAPPED_GROUPS_VALUE_KIND => {
            conduit_text::MAXIMUM_MORSE_GAPPED_GROUPS_BYTES as u32
        }
        conduit_text::MORSE_SYMBOLS_VALUE_KIND => conduit_text::MAXIMUM_MORSE_SYMBOLS_BYTES as u32,
        conduit_text::MORSE_PATTERN_VALUE_KIND => conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        _ => CORD_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offer::CpuFeatures;

    #[test]
    fn shared_caller_produces_distinct_direct_and_recursive_native_plans() {
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
        assert_ne!(
            prepared.direct.expanded_form_id,
            prepared.recursive.expanded_form_id
        );
        assert_ne!(prepared.direct.plan_id, prepared.recursive.plan_id);
        assert_eq!(prepared.direct.fragments[0].placements.len(), 3);
        assert_eq!(prepared.direct.fragments[0].connections.len(), 2);
        assert_eq!(prepared.recursive.fragments[0].placements.len(), 7);
        assert_eq!(prepared.recursive.fragments[0].connections.len(), 6);
        let recursive_implementations = prepared.recursive.fragments[0]
            .placements
            .iter()
            .map(|placement| placement.implementation_id.as_str())
            .collect::<alloc::vec::Vec<_>>();
        for implementation in [
            crate::offer::TEXT_CHARACTERS_IMPLEMENTATION,
            crate::offer::MORSE_LOOKUP_IMPLEMENTATION,
            crate::offer::MORSE_INTERSPERSE_IMPLEMENTATION,
            crate::offer::MORSE_FLATTEN_IMPLEMENTATION,
            crate::offer::MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION,
        ] {
            assert!(recursive_implementations.contains(&implementation));
        }
    }
}
