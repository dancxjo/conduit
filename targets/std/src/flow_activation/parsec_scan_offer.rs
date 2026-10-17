//! Exact finite source-result scan offer and selected initial Form.
use super::parsec_combine::{command_kind, parsec_combine_kind, result_kind, KIND};
use conduit_core::{
    Back, BackOfferBuilder, CapabilityOffer, CheckedValueContract, PlannedGear,
    PlannedScanActivation,
};
use conduit_face::{
    ParsecMapCommand, ParsecMapState, ParsecMapTransitionResult, PARSEC_MAP_FRAME_BYTES,
};
use conduit_plot::{rust_binding::NativeRustBinding, ProfileCatalog, StartupCatalog};
pub const MAXIMUM_ITEMS: u16 = 32;
pub const PARSEC_SCAN_SOURCE: &str = include_str!("parsec_scan.conduit");

pub fn parsec_initial_result() -> Result<ParsecMapTransitionResult, String> {
    let state = ParsecMapState::new(false, 1, false)
        .map_err(|e| format!("Parsec initial state law: {e:?}"))?;
    ParsecMapTransitionResult::new(false, state)
        .map_err(|e| format!("Parsec initial result law: {e:?}"))
}
pub fn parsec_initial_bytes() -> Result<Vec<u8>, String> {
    parsec_initial_result()?
        .into_structured()
        .map_err(|e| format!("{e:?}"))?
        .canonical_bytes()
        .map_err(|e| format!("{e:?}"))
}
pub fn install_parsec_scan_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_structured_type(
        "ParsecMapCommand",
        ParsecMapCommand::semantic_type().map_err(|e| format!("{e:?}"))?,
    )?;
    startup.insert_structured_type(
        "ParsecMapTransitionResult",
        ParsecMapTransitionResult::semantic_type().map_err(|e| format!("{e:?}"))?,
    )?;
    startup.insert(conduit_plot::KindSignature {
        kind: KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(parsec_combine_kind()?)
        .map_err(|e| format!("{e:?}"))?;
    startup.insert_exact_initial_info(
        result_kind()?,
        conduit_plot::text_startup_literal("Pregame Fire"),
        parsec_initial_bytes()?,
        |bytes| {
            conduit_core::StructuredInfoValue::from_canonical_bytes(bytes)
                .ok()
                .and_then(|value| ParsecMapTransitionResult::from_structured(value).ok())
                .is_some()
        },
    )
}
pub fn parsec_scan_offer(maximum_items: u16) -> Result<CapabilityOffer, String> {
    if maximum_items == 0 || maximum_items > MAXIMUM_ITEMS {
        return Err("Parsec scan admits1..=32 commands per Play".into());
    }
    let item = CheckedValueContract::new(command_kind()?, PARSEC_MAP_FRAME_BYTES as u32, vec![])
        .map_err(|e| format!("{e:?}"))?;
    let accumulator =
        CheckedValueContract::new(result_kind()?, PARSEC_MAP_FRAME_BYTES as u32, vec![])
            .map_err(|e| format!("{e:?}"))?;
    let kind = conduit_semantic_catalog::flow_scan_semantic_contract(
        &item,
        &accumulator,
        &parsec_initial_bytes()?,
        None,
        maximum_items,
    )
    .map_err(str::to_string)?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: "std/parsec-map-scan@1".into(),
            execution_profile_id: "conduit.std/parsec-map-scan@1".into(),
            implementation_id: "std/kernel-parsec-map-scan@1".into(),
            artifact_id: "conduit-std-host/parsec-map-scan@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub(crate) fn validate_planned_parsec_scan(
    owner: &PlannedGear,
    planned: &PlannedScanActivation,
) -> Result<(), String> {
    if planned.initial_accumulator != parsec_initial_bytes()? {
        return Err("Parsec scan has a different initial source Form".into());
    }
    let expected = parsec_scan_offer(planned.limits.maximum_items)?;
    if owner.kind_id != expected.kind_id
        || owner.kind_contract_revision != expected.kind_contract_revision
        || owner.capability_id != expected.capability_id
        || owner.execution_profile_id != expected.implementation.execution_profile_id
        || owner.implementation_id != expected.implementation.implementation_id
        || owner.artifact_id != expected.implementation.artifact_id
        || owner.inputs != expected.inputs
        || owner.outputs != expected.outputs
        || owner.semantic_contract != expected.semantic_contract
        || owner.limits != expected.limits
        || !owner.configuration.is_empty()
        || owner.base.is_some()
        || !owner.realization_characteristics.is_empty()
        || !owner.realization_properties.is_empty()
        || !owner.terminal_transductions.is_empty()
        || !owner.host_calls.is_empty()
        || !owner.resources.is_empty()
        || !owner.authority.is_empty()
        || !owner.pool_references.is_empty()
    {
        return Err("Parsec scan differs from exact installed Back".into());
    }
    Ok(())
}
