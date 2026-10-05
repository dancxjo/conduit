//! One portable inventory shared by source checking and authoring discovery.
use crate::*;
use alloc::vec::Vec;
use conduit_core::Kind;

pub(crate) fn nucleus_kind(contract: StandardKindContract, revision: &str) -> Kind {
    match contract.kind_id.as_str() {
        MATH_CLAMP_KIND => math_clamp_semantic_contract(),
        MATH_SCALE_KIND => math_scale_semantic_contract(),
        MATH_DEADBAND_KIND => math_deadband_semantic_contract(),
        FIRST_KIND => flow_first_scalar_semantic_contract(),
        AUDIO_TONE_KIND => audio_tone_semantic_contract(),
        _ => contract.into_semantic_contract(revision),
    }
}

pub(crate) fn palette_semantics() -> Vec<(StandardKindContract, Kind)> {
    let mut entries = supported_nucleus_contracts_with_revisions()
        .into_iter()
        .map(|(description, revision)| {
            let kind = nucleus_kind(description.clone(), revision);
            (description, kind)
        })
        .collect::<Vec<_>>();
    entries.extend(
        patchbay_presentation_contracts()
            .into_iter()
            .map(|description| {
                let kind = description
                    .clone()
                    .into_semantic_contract(PATCHBAY_PRESENTATION_REVISION);
                (description, kind)
            }),
    );
    entries.extend([
        (orbium_seed_contract(), orbium_seed_semantic_contract()),
        (lenia_step_contract(), lenia_step_semantic_contract()),
        (
            scalar_field_presentation_contract(),
            scalar_field_presentation_semantic_contract(),
        ),
    ]);
    entries.extend(robotics_hazard_contracts_with_revisions().into_iter().map(
        |(description, revision)| {
            let kind = description.clone().into_semantic_contract(revision);
            (description, kind)
        },
    ));
    entries.push((keyboard_contract(), keyboard_semantic_contract()));
    entries.extend(application_contracts().into_iter().map(|description| {
        let kind = application_semantic_contract(description.clone());
        (description, kind)
    }));
    entries.extend([
        (
            http_client_contract(),
            conduit_web::http_client_semantics().into_semantic_contract(),
        ),
        (
            http_server_contract(),
            conduit_web::http_server_semantics().into_semantic_contract(),
        ),
    ]);
    entries
}

/// Discovery includes portable Kinds with no currently installed Host Back.
pub fn palette_contracts() -> Vec<StandardKindContract> {
    palette_semantics()
        .into_iter()
        .map(|(description, _)| description)
        .collect()
}

#[cfg(feature = "plot-catalog")]
pub fn standard_profile_catalog() -> conduit_plot::ProfileCatalog {
    let mut catalog = conduit_plot::ProfileCatalog::new();
    for (contract, revision) in supported_nucleus_contracts_with_revisions() {
        catalog
            .insert_kind(nucleus_kind(contract, revision))
            .expect("standard catalog kinds are unique");
    }
    catalog
}
