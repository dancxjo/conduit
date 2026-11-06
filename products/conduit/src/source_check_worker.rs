//! Selected product compiler context, retained privately inside a bounded child.
/// Actual product catalogue construction. Revision labels describe this selected
/// artifact's context; they are never a catalogue lookup or an execution grant.
pub fn standard_context() -> Result<conduit_plot::SourceCheckContext, String> {
    use conduit_plot::{CanonicalBackCatalog, CheckContextRevision, SourceCheckContext};
    let (startup, profiles) = crate::plot_source::standard_catalogs()?;
    SourceCheckContext::new(
        CheckContextRevision {
            grammar_edition: "conduitese-v1".into(),
            catalog: "conduit/product-standard@1".into(),
            kinds_fores: "conduit/product-standard@1".into(),
            types_laws: "conduit/product-standard@1".into(),
            backs: "empty@1".into(),
        },
        startup,
        profiles,
        CanonicalBackCatalog::new(),
    )
    .map_err(|error| format!("Source-check context: {error:?}"))
}
/// Dedicated process entrance; does not install a Host offer or execute a Plot.
#[cfg(target_os = "linux")]
pub fn run(address_space_bytes: u64, cpu_seconds: u64) -> Result<(), String> {
    use conduit_std_host::source_check::process::{self, SourceCheckProcessLimits};
    process::run(
        SourceCheckProcessLimits {
            address_space_bytes,
            cpu_seconds,
        },
        standard_context,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    )
}
#[cfg(not(target_os = "linux"))]
pub fn run(_: u64, _: u64) -> Result<(), String> {
    Err("bounded compiler process requires Linux".into())
}
