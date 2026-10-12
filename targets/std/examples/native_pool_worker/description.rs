//! Describe one ordinary locally planned model Back before pool admission.
//! No provider is executed, occupied, selected from a pool, or warmed here.
use conduit_core::{Plan, PoolRealizationEnvelope};
use conduit_std_host::StdHost;

pub(super) fn describe(
    host: &StdHost,
    source: &str,
    entry: &str,
) -> Result<(Plan, PoolRealizationEnvelope), String> {
    if source.len() > 16_384 || entry.is_empty() || entry.len() > 256 {
        return Err("member description Source extent refused".into());
    }
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)
        .map_err(|e| e.to_string())?;
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .map_err(|error| {
                format!(
                    "member description Source: {} {}",
                    error.code, error.message
                )
            })?;
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &profiles)
        .map_err(|_| "member description Source failed expansion")?;
    if expanded.expanded.gears.len() != 1
        || expanded.expanded.gears[0].kind_id.as_str() != conduit_ai::LLM_GENERATE_KIND
    {
        return Err("member description requires one checked generation Gear".into());
    }
    let plan = host
        .plan_expanded_local(&expanded.expanded)
        .map_err(|e| e.to_string())?;
    if !conduit_core::verify_plan(&plan)
        || plan.fragments.len() != 1
        || plan.fragments[0].placements.len() != 1
    {
        return Err("member description Plan shape refused".into());
    }
    let placement = &plan.fragments[0].placements[0];
    let realization = PoolRealizationEnvelope {
        host_id: placement.host_id.clone(),
        boot_id: placement.boot_id.clone(),
        offer_generation: placement.offer_generation,
        capability_id: placement.capability_id.clone(),
        implementation_id: placement.implementation_id.clone(),
        artifact_id: placement.artifact_id.clone(),
        member_capacity: 1,
        resources: placement.resources.clone(),
        admitted_lines: vec![],
    };
    Ok((plan, realization))
}
