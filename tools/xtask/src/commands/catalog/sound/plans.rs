use conduit_core::{
    resource_offer, BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, PROTOCOL_VERSION,
};
use serde::Serialize;

use super::{forms, CatalogError};

#[derive(Debug, Serialize)]
pub(super) struct PlanComparison {
    invariant: &'static str,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    plans: Vec<PlanSpecimen>,
}

#[derive(Debug, Serialize)]
struct PlanSpecimen {
    realization: &'static str,
    plan_id: String,
    host_id: String,
    boot_id: String,
    implementation_id: String,
    execution_profile_id: String,
}

pub(super) fn build() -> Result<PlanComparison, CatalogError> {
    let (_, catalog) = conduit_pete::catalogs()
        .map_err(|error| CatalogError::new("sound-plan-catalog-invalid", error))?;
    let form = conduit_form::parse(forms::SIMPLE_FORM, &catalog)
        .map_err(|error| CatalogError::new("sound-plan-form-invalid", error.to_string()))?;
    let create = create_plan()?;
    let opl = opl_plan(&form)?;
    if create.source_document_id != form.source_document_id
        || create.checked_form_id != form.checked_form_id
        || create.expanded_form_id != form.expanded_form_id
        || opl.source_document_id != form.source_document_id
        || opl.checked_form_id != form.checked_form_id
        || opl.expanded_form_id != form.expanded_form_id
        || create.plan_id == opl.plan_id
    {
        return Err(CatalogError::new(
            "sound-plan-identity-invariant-failed",
            "cross-realization Plans did not preserve one Form and distinct Plans",
        ));
    }
    Ok(PlanComparison {
        invariant: "same-source-checked-expanded-form-distinct-exact-plans",
        source_document_id: form.source_document_id.as_str().to_owned(),
        checked_form_id: form.checked_form_id.as_str().to_owned(),
        expanded_form_id: form.expanded_form_id.as_str().to_owned(),
        plans: vec![
            specimen("pete-create-oi", &create)?,
            specimen("adlib-opl2", &opl)?,
        ],
    })
}

fn create_plan() -> Result<conduit_core::Plan, CatalogError> {
    let observation = conduit_pete::CreateSpeakerObservation {
        host_id: HostId::from("conformance-pete"),
        boot_id: BootId::from("conformance-pete-boot"),
        offer_generation: OfferGeneration(1),
        serial_base_id: "conformance/pete/create1/serial/0".into(),
        robot_identity: "conformance/pete/create1/robot".into(),
        robot_identity_verified: true,
        speaker_resource_id: "conformance/pete/create1/speaker".into(),
        mode: conduit_pete::OiMode::Safe,
        currently_usable: true,
    };
    conduit_pete::simple_melody_plan(&observation, true)
        .map_err(|error| CatalogError::new("create-plan-failed", error.to_string()))
}

fn opl_plan(form: &conduit_form::CheckedForm) -> Result<conduit_core::Plan, CatalogError> {
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("conformance-opl2"),
        boot_id: BootId::from("conformance-opl2-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conformance/opl2@1"),
        resources: vec![resource_offer(
            "conformance-opl2-memory",
            conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
            1_048_576,
        )],
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    };
    conduitos::opl2_offer::append_to_advertisement(
        &mut host,
        conduitos::opl2_offer::Opl2Offer {
            artifact_build: "conformance-build",
            realization: conduitos::opl2_offer::Opl2Realization {
                base_id: [2; 32],
                clock_hz: conduitos::opl2_offer::OPL2_CLOCK_HZ,
                channels: conduitos::opl2_offer::OPL2_CHANNELS,
                maximum_error_parts_per_million: 2_500,
                event_slots: 16,
                register_write_slots: 400,
                patch_profile: conduitos::opl2_offer::OPL2_PATCH_PROFILE,
            },
        },
        "conformance-build",
    )
    .map_err(|error| CatalogError::new("opl2-offer-invalid", format!("{error:?}")))?;
    let placements = conduit_planner::default_placements(form, core::slice::from_ref(&host))
        .map_err(|error| CatalogError::new("opl2-placement-failed", error.to_string()))?;
    conduit_planner::plan(
        form,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .map_err(|error| CatalogError::new("opl2-plan-failed", error.to_string()))
}

fn specimen(
    realization: &'static str,
    plan: &conduit_core::Plan,
) -> Result<PlanSpecimen, CatalogError> {
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::MUSIC_PLAY_KIND)
        .ok_or_else(|| {
            CatalogError::new(
                "sound-plan-missing-output",
                "Plan has no music/play placement",
            )
        })?;
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.placements.contains(placement))
        .ok_or_else(|| CatalogError::new("sound-plan-missing-fragment", realization))?;
    Ok(PlanSpecimen {
        realization,
        plan_id: plan.plan_id.as_str().to_owned(),
        host_id: fragment.host_id.as_str().to_owned(),
        boot_id: fragment.boot_id.as_str().to_owned(),
        implementation_id: placement.implementation_id.as_str().to_owned(),
        execution_profile_id: placement.execution_profile_id.as_str().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_form_has_distinct_create_and_opl_plans() {
        let comparison = build().unwrap();
        assert_eq!(comparison.plans.len(), 2);
        assert_ne!(comparison.plans[0].plan_id, comparison.plans[1].plan_id);
        assert_ne!(comparison.plans[0].host_id, comparison.plans[1].host_id);
        assert_ne!(
            comparison.plans[0].implementation_id,
            comparison.plans[1].implementation_id
        );
    }
}
