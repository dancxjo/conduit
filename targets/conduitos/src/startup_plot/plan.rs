//! Check and plan the unchanged shipped startup plot against native offers.
use crate::{identity::BootIdentities, offer::HostOffer};
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};
use conduit_plot::*;

pub(super) const WAKE: &str = "conduitos/body-wake@1";
pub(super) const CHIME: &str = "conduitos/startup-chime@1";
pub(super) const CALL: &str = "conduit.host/native-startup-chime@1";
const RESOURCE: &str = "conduit.resource/native-audio-cue-slot@1";
pub(super) fn prepare(
    ids: &BootIdentities,
    host: &HostOffer<'_>,
    build: &str,
) -> Result<Plan, &'static str> {
    let mut advertisement =
        crate::ordinary_plan::advertisement(ids, host, build).map_err(|e| e.as_str())?;
    let mut source = offer(false);
    let mut sink = offer(true);
    source.implementation.artifact_id = ArtifactId::from(build);
    sink.implementation.artifact_id = ArtifactId::from(build);
    advertisement.capabilities.extend([source, sink]);
    advertisement
        .resources
        .push(resource_offer("conduitos/audio-cue", RESOURCE, 1));
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_body_startup_catalogs(&mut startup, &mut profile)
        .map_err(|_| "startup-catalog")?;
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../../../plots/startup-chime/main.conduit")),
        &startup,
    )
    .map_err(|_| "startup-source-check")?;
    let plot = expand_canonical_plot(&checked, "startup_chime", &profile)
        .map_err(|_| "startup-source-expand")?;
    let hosts = [advertisement];
    let placements = default_expanded_placements(&plot, &hosts).map_err(|_| "startup-placement")?;
    let plan = plan_expanded_canonical_with_options(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: BOOL_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| "startup-plan")?;
    if !verify_plan(&plan)
        || plan.fragments.len() != 1
        || plan.fragments[0].placements.len() != 2
        || plan.fragments[0].connections.len() != 1
    {
        return Err("startup-plan-shape");
    }
    Ok(plan)
}
fn offer(sink: bool) -> CapabilityOffer {
    let implementation = if sink { CHIME } else { WAKE };
    conduit_semantic_catalog::realization_offer(
        if sink {
            conduit_semantic_catalog::startup_chime_contract()
        } else {
            conduit_semantic_catalog::body_wake_contract(false)
        },
        if sink {
            conduit_semantic_catalog::STARTUP_CHIME_REVISION
        } else {
            conduit_semantic_catalog::BODY_WAKE_REVISION
        },
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: implementation,
            implementation,
            artifact: "conduitos/native-startup@1",
        },
        if sink {
            vec![HostCallRequirement {
                contract_id: CALL.into(),
                target_kind: Some(kind_id("sound/optional-audible-cue")),
                maximum_in_flight: 1,
                maximum_input_bytes: BOOL_ENCODED_LEN as u32,
                maximum_output_bytes: 0,
            }]
        } else {
            vec![]
        },
        if sink {
            vec![resource_requirement(RESOURCE, 1)]
        } else {
            vec![]
        },
        vec![],
    )
}
