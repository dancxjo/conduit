//! Selected product compiler context, retained privately inside a bounded child.
mod definition;
pub use definition::{derive_image_definition, SourceCheckProviderIdentity};
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

/// Construct bounded preparation machinery from actual checked equipment intent.
/// Does not launch, admit memory, authorize Source, or install a Ready offer.
/// The Root must independently admit preparation and preserve artifact custody.
#[cfg(target_os = "linux")]
pub fn from_configuration(
    checked: &conduit_host_make::CheckedHostConfiguration,
) -> Result<conduit_std_host::source_check::process::SourceCheckProcessWorker, String> {
    use conduit_std_host::source_check::process::{
        SourceCheckProcessLimits, SourceCheckProcessWorker,
    };
    if !cfg!(target_arch = "x86_64") {
        return Err(
            "selected Source compiler has no qualified adapter for this running architecture"
                .into(),
        );
    }
    let selected =
        conduit_workspace_make::CheckedSourceCheckInitialization::from_configuration(checked)?;
    SourceCheckProcessWorker::new(
        selected.executable,
        SourceCheckProcessLimits {
            address_space_bytes: selected.address_space_bytes,
            cpu_seconds: selected.cpu_seconds,
        },
        std::time::Duration::from_millis(selected.deadline_milliseconds),
        selected.maximum_startup_bytes as usize,
    )
    .and_then(|worker| {
        worker.with_executable_identity(
            selected.executable_sha256,
            selected.maximum_executable_bytes,
        )
    })
    .and_then(|worker| worker.with_reply_bound(selected.reply_bytes as usize))
    .map_err(|error| format!("selected Source compiler: {error:?}"))
}

#[cfg(target_os = "linux")]
mod installation;
#[cfg(target_os = "linux")]
pub use installation::{
    initialize_image, initialize_image_with_caller_workspace,
    initialize_image_with_prepared_creche, InitializedSourceCheckImage,
    SourceCheckImageInstallation,
};
