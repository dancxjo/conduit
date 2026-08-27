//! Ordinary planning for one exact boot-scoped portable keyboard source.

use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::{
    ActivePlayIdentity, BaseImplementationId, HostAdvertisement, Plan, bind_active_play,
};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};

use crate::{
    identity::BootIdentities,
    keyboard_offer::{
        CONTROLLER_RESOURCE, DEVICE_RESOURCE, ENDPOINT_RESOURCE, INTERFACE_RESOURCE,
        KEYBOARD_EXECUTION_PROFILE, KEYBOARD_IMPLEMENTATION, OPERATION_RESOURCE, REPORT_RESOURCE,
        TRANSITION_RESOURCE,
    },
    offer::HostOffer,
    ordinary_plan::{PreparationError, advertisement},
};

pub const KEYBOARD_FORM_SOURCE: &str =
    "form conduitos-keyboard {\n    keyboard: input/keyboard\n}\n";

pub struct PreparedKeyboardPlay {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub active_play: ActivePlayIdentity,
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedKeyboardPlay, PreparationError> {
    let advertisement = advertisement(identities, offer, build_id)?;
    if offer.keyboard.is_none() {
        return Err(PreparationError::PlacementRejected);
    }
    let form = checked_expanded_form()?;
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
            connection_item_capacity: conduit_std_catalog::KEYBOARD_MAX_QUEUE_ITEMS,
            connection_byte_capacity: conduit_std_catalog::KEYBOARD_MAX_QUEUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    validate(&plan, &advertisement, offer, build_id)?;
    let fragment = &plan.fragments[0];
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    Ok(PreparedKeyboardPlay {
        advertisement,
        plan,
        active_play,
    })
}

pub fn validate(
    plan: &Plan,
    advertisement: &HostAdvertisement,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<(), PreparationError> {
    let Some(keyboard) = offer.keyboard else {
        return Err(PreparationError::OfferMismatch);
    };
    keyboard
        .validate(build_id)
        .map_err(|_| PreparationError::OfferMismatch)?;
    if !conduit_core::verify_plan(plan)
        || plan.fragments.len() != 1
        || advertisement.host_id.as_str() != crate::identity::hex(&offer.host_id)
        || advertisement.boot_id.as_str() != crate::identity::hex(&offer.boot_id)
        || advertisement.offer_generation.0 != offer.generation
    {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    if fragment.host_id != advertisement.host_id
        || fragment.boot_id != advertisement.boot_id
        || fragment.offer_generation != advertisement.offer_generation
        || fragment.placements.len() != 1
        || !fragment.connections.is_empty()
    {
        return Err(PreparationError::PlanRejected);
    }
    let placement = &fragment.placements[0];
    if placement.kind_id.as_str() != conduit_std_catalog::KEYBOARD_KIND
        || placement.kind_contract_revision != conduit_std_catalog::keyboard_contract_revision()
        || placement.execution_profile_id.as_str() != KEYBOARD_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != KEYBOARD_IMPLEMENTATION
        || placement.artifact_id.as_str() != alloc::format!("conduitos-build/{build_id}")
        || !placement.inputs.is_empty()
        || placement.outputs != conduit_std_catalog::keyboard_outputs()
    {
        return Err(PreparationError::PlanRejected);
    }
    let expected_classes = [
        "conduit.resource/runtime-memory@1",
        CONTROLLER_RESOURCE,
        DEVICE_RESOURCE,
        INTERFACE_RESOURCE,
        ENDPOINT_RESOURCE,
        REPORT_RESOURCE,
        TRANSITION_RESOURCE,
        OPERATION_RESOURCE,
    ];
    if placement.resources.len() != expected_classes.len()
        || expected_classes.iter().any(|class| {
            placement
                .resources
                .iter()
                .filter(|binding| binding.class_id.as_str() == *class && binding.units > 0)
                .count()
                != 1
        })
    {
        return Err(PreparationError::PlanRejected);
    }
    let resource_pool_ids = advertisement
        .resources
        .iter()
        .map(|resource| &resource.pool_id)
        .collect::<Vec<_>>();
    if placement
        .resources
        .iter()
        .any(|binding| !resource_pool_ids.contains(&&binding.pool_id))
    {
        return Err(PreparationError::PlanRejected);
    }
    Ok(())
}

fn checked_expanded_form() -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(KEYBOARD_FORM_SOURCE);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form(&checked, "conduitos-keyboard", &profile)
        .map_err(|_| PreparationError::FormRejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        keyboard_offer::KeyboardRealization,
        offer::{CpuFeatures, HostOffer},
    };

    fn fixture() -> (BootIdentities, HostOffer<'static>) {
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
            1_048_576,
        )
        .with_keyboard(
            KeyboardRealization {
                controller_id: [3; 32],
                device_id: [4; 32],
                interface_id: [5; 32],
                endpoint_id: [6; 32],
                report_buffers: 2,
                transition_slots: 8,
                operation_slots: 2,
            },
            "build",
        )
        .unwrap();
        (identities, offer)
    }

    #[test]
    fn exact_ready_chain_produces_one_ordinary_keyboard_placement() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        assert_eq!(prepared.plan.fragments[0].placements.len(), 1);
        assert_eq!(prepared.plan.fragments[0].placements[0].resources.len(), 8);
    }

    #[test]
    fn absent_stale_exhausted_and_mismatched_truth_refuse() {
        let (identities, offer) = fixture();
        let absent = HostOffer::new(
            &identities,
            "build",
            offer.cpu_features,
            offer.runtime_arena_bytes,
        );
        assert!(prepare(&identities, &absent, "build").is_err());

        let prepared = prepare(&identities, &offer, "build").unwrap();
        let mut stale = offer;
        stale.boot_id = [9; 32];
        assert_eq!(
            validate(&prepared.plan, &prepared.advertisement, &stale, "build"),
            Err(PreparationError::PlanRejected)
        );
        let mut mismatched = fixture().1;
        mismatched.keyboard.as_mut().unwrap().artifact_build = "other";
        assert_eq!(
            prepare(&identities, &mismatched, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
        let mut exhausted = fixture().1;
        exhausted
            .keyboard
            .as_mut()
            .unwrap()
            .realization
            .report_buffers = 0;
        assert_eq!(
            prepare(&identities, &exhausted, "build").err(),
            Some(PreparationError::OfferMismatch)
        );
    }
}
