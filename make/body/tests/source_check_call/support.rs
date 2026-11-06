use super::*;
pub fn owner(source: &str, generation: u64) -> (SourceCheckProjection, SourceCheckRequest) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    let context = SourceCheckContext::new(
        CheckContextRevision {
            grammar_edition: "conduitese-v1".into(),
            catalog: "fixture/source-check@1".into(),
            kinds_fores: "reviewed/text-pipeline@1".into(),
            types_laws: "reviewed/text-types@1".into(),
            backs: "empty@1".into(),
        },
        startup,
        profiles,
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let owner =
        SourceCheckProjection::new("fixture/authorized-source", generation, source, context)
            .unwrap();
    let request = SourceCheckRequest {
        snapshot: owner.snapshot().clone(),
        context: owner.context().clone(),
        entry: "hello".into(),
        limits: SourceCheckLimits::default(),
    };
    (owner, request)
}
#[path = "support/runtime.rs"]
mod runtime;
pub use runtime::{definition, plan};
