//! Exact installed read-only Source inventory, independent of runnable worksets.
//! Preparation checks meaning only; no Host offers, Plan, Play or device access.
use super::WorksetRefusal;
use conduit_plot::{ProfileCatalog, StartupCatalog};
use conduitos_application_core::data::Target;

const VISION_SOURCE: &str = include_str!("../../../../plots/vision/main.conduit");
const SOURCE_BYTES: usize = 8192;
const _: () = assert!(VISION_SOURCE.len() <= SOURCE_BYTES);

/// This product profile explicitly permits inspection of this one embedded asset.
/// No path, URL or guest-selected bytes are accepted. Call only during preparation.
pub(super) fn vision(body: conduit_core::PlanId) -> Result<Target, WorksetRefusal> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(VISION_SOURCE),
        &startup,
    )
    .map_err(|_| WorksetRefusal::Catalog)?;
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(&checked, "vision", &profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    let graph = patchbay_graph::PatchbayGraph::from_authoring(&expanded)
        .map_err(|_| WorksetRefusal::Plan)?;
    Ok(Target {
        graph,
        plan: None,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_inspection_has_real_source_boundaries_and_no_realization() {
        let target = vision("plan/inspecting-body".into()).unwrap();
        assert!(target.plan.is_none());
        let heap_witness: patchbay_graph::PatchbayGraph = serde_json::from_str(include_str!(
            "../../application-core/tests/fixtures/vision-graph.json"
        ))
        .unwrap();
        assert_eq!(target.graph, heap_witness);
        assert_eq!(target.graph.plot_name, "vision");
        assert_eq!(target.graph.gears.len(), 8);
        assert_eq!(target.graph.cords.len(), 16);
        assert_eq!(target.graph.front_inputs.len(), 1);
        assert_eq!(target.graph.front_outputs.len(), 1);
        assert_eq!(
            target.graph.source_document_id.as_str(),
            "5345dc154d1209167735c7d9e37a725c9a470a239fd7764e2986aacd58af4382"
        );
        let wire = conduitos_application_core::data::encode_command(
            &conduitos_application_core::data::Command::PrepareTarget {
                slot: 3,
                target: 3,
                value: target,
            },
        )
        .unwrap();
        assert!(wire.len() <= conduitos_application_core::MAX_BLOB);
    }
}
