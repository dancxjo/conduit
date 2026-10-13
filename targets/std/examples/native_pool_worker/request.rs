//! Shared finite checked generation meaning for execution and forecast queries.
use conduit_core::ValuePayload;
use conduit_plot::CheckedGear;
pub(super) fn check(
    source: &str,
    entry: &str,
    input: &ValuePayload,
) -> Result<CheckedGear, String> {
    if source.len() > 16_384 || entry.is_empty() || entry.len() > 256 {
        return Err("generation Source extent refused".into());
    }
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)
        .map_err(|e| e.to_string())?;
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .map_err(|_| "operation Source failed checking")?;
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &profiles)
        .map_err(|_| "operation Source failed expansion")?;
    if expanded.expanded.gears.len() != 1 {
        return Err("operation requires one checked model Gear".into());
    }
    let gear = &expanded.expanded.gears[0];
    if gear.kind_id.as_str() != conduit_ai::LLM_GENERATE_KIND
        || gear.inputs.len() != 1
        || input.value_kind.as_str() != conduit_ai::GENERATION_REQUEST_VALUE_KIND
    {
        return Err("operation input semantic type refused".into());
    }
    Ok(gear.clone())
}
