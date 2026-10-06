//! Finite local-model proof planning and ordinary Host execution.
use super::*;

pub(super) fn run_profile(
    host: &mut StdHost,
    profile: LocalModelKindProfile,
) -> Result<(String, bool), Box<dyn std::error::Error>> {
    const PROOF_MAXIMUM_INPUT_BYTES: u64 = 16 * 1024;
    let contract = conduit_ai::llm_contract(profile.kind()).expect("proof profiles are L0");
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    crate::installed_std::test_local_model_io::install_catalog(
        &mut startup,
        &mut profiles,
        contract.inputs[0].value_kind.as_str(),
        contract.outputs[0].value_kind.as_str(),
    );
    let provider_call = host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == profile.kind())
        .and_then(|offer| {
            offer.host_calls.iter().find(|call| {
                call.target_kind
                    .as_ref()
                    .is_some_and(|kind| kind.as_str() == profile.kind())
            })
        })
        .ok_or("local-model proof Back has no Host Call bound")?;
    let maximum_input_bytes = contract
        .bounds
        .maximum_input_bytes()
        .min(PROOF_MAXIMUM_INPUT_BYTES)
        .min(u64::from(provider_call.maximum_input_bytes));
    let maximum_output_bytes = contract
        .bounds
        .maximum_output_bytes()
        .min(u64::from(provider_call.maximum_output_bytes));
    let maximum_work_units = contract.bounds.maximum_work_units();
    let source = format!(
        "plot run {{\n source: conduit-test/local-model-request\n model: {}({}, 1, {}, {}, 0)\n sink: conduit-test/local-model-result\n source.value >> model.request\n model.result >> sink.value\n}}\n",
        profile.kind(), maximum_input_bytes, maximum_output_bytes, maximum_work_units,
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &startup).map_err(|error| {
            format!(
                "local-model proof Plot check: {} {}",
                error.code, error.message
            )
        })?;
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "run", &profiles).map_err(|error| {
            format!(
                "local-model proof expansion: {} {}",
                error.code, error.message
            )
        })?;
    let connection_byte_capacity = u32::try_from(maximum_input_bytes.max(maximum_output_bytes))?;
    run_expanded(host, expanded, profile.kind(), connection_byte_capacity)
}

pub(super) fn run_expanded(
    host: &mut StdHost,
    expanded: conduit_plot::ExpandedCanonicalPlot,
    proof_name: &str,
    connection_byte_capacity: u32,
) -> Result<(String, bool), Box<dyn std::error::Error>> {
    let hosts = vec![host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)?;
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )?;
    let plan_id = plan.plan_id.as_str().to_string();
    let fragment = plan
        .fragments
        .into_iter()
        .next()
        .ok_or("local-model proof Plan has no fragment")?;
    let connection_limits = fragment
        .connections
        .iter()
        .map(|connection| (connection.item_capacity, connection.byte_capacity))
        .collect::<Vec<_>>();
    let host_call_limits = fragment
        .placements
        .iter()
        .map(|placement| {
            (
                placement.kind_id.as_str().to_string(),
                placement
                    .host_calls
                    .iter()
                    .map(|call| (call.maximum_input_bytes, call.maximum_output_bytes))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    let report = host
        .run_fragment_to(fragment, &mut output, &mut NoopTimer)
        .map_err(|error| {
            format!(
                "{} live Plan/Play: {error}; connection limits {connection_limits:?}; host-call limits {host_call_limits:?}",
                proof_name
            )
        })?;
    Ok((plan_id, report.kernel.is_some()))
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
