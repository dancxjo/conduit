//! Original compiler, caller and optional ambient worker IMAGE construction.
use super::*;
#[cfg(target_os = "linux")]
pub(crate) struct PreparedFileSource {
    pub(super) caller_workspace_bytes: usize,
    pub(crate) host: conduit_std_host::StdHost,
    pub(crate) source: ImagePreparedValue<OwnedSourceCheckInput>,
    pub(crate) caller: ImagePreparedValue<()>,
}

#[cfg(target_os = "linux")]
pub(in crate::durable_host) fn prepare_with_ambient(
    selection: &Selection,
    installation: &Installation,
    config: conduit_std_host::StdHostConfig,
    creche: Option<conduit_core::ResourceOffer>,
    ambient: Option<&super::super::ambient_policy::Selection>,
) -> Result<PreparedFileSource, String> {
    use conduit::source_check_worker::*;
    use conduit_core::*;
    use conduit_std_host::{ImmediateSourcePreparationBudget, RunControl};
    use conduit_std_offers::*;
    validate_selection(selection, installation)?;
    let selected = installation
        .selected_host_configuration
        .as_ref()
        .ok_or("missing selected IMAGE")?;
    let checked_host = selected.checked()?;
    let image = selected.host_image().ok_or("missing selected IMAGE")?;
    let text_file = selection
        .text_generation
        .as_ref()
        .map(|selected| {
            super::super::text_file_policy::prepare(selected, &config, image, &checked_host)
        })
        .transpose()?;
    // Actual descriptor access is supplied by the independently selected local
    // OS policy. Configuration labels and network clients cannot open it.
    let file =
        fs::File::open(&selection.file).map_err(|e| format!("open owner-selected File: {e}"))?;
    let grant = AuthorityGrant {
        grant_id: fresh_identity("grant/local-source-file", &selection.snapshot.sha256)
            .as_str()
            .into(),
        contract_id: SOURCE_CHECK_AUTHORITY.into(),
        host_call_contract_id: SOURCE_CHECK_CALL.into(),
        subject_kind: SOURCE_CHECK_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: SOURCE_CHECK_IMPLEMENTATION.into(),
    };
    let provider = SourceCheckProviderIdentity {
        instance: fresh_identity("base/local-source-compiler", &selection.configuration_id)
            .as_str()
            .into(),
        generation: config.offer_generation.0,
        resource_generation: ResourceGenerationId(fresh_identity(
            "resource/local-source-compiler",
            &selection.configuration_id,
        )),
    };
    let control = RunControl::default();
    let catalog = conduit_workspace_make::catalog();
    // SAFETY: the stopped local OS selection supplies the actual File and
    // original Root authority. Explicit reviewed preparation envelopes remain
    // the caller's qualification obligation; the IMAGE counter must admit them.
    let ambient = ambient
        .map(|policy| policy.source_installation(&config, &checked_host))
        .transpose()?;
    let initialized = unsafe {
        initialize_image_with_ambient(
            SourceCheckImageInstallation {
                config,
                image,
                checked: &checked_host,
                catalog: &catalog,
                grant,
                provider,
                file,
                maximum_source_bytes: selection.maximum_source_bytes,
                capture_budget: ImmediateSourcePreparationBudget {
                    preparation_bytes: selection
                        .capture_preparation_bytes
                        .try_into()
                        .map_err(|_| "Source capture preparation overflow")?,
                    retained_bytes: selection
                        .capture_retained_bytes
                        .try_into()
                        .map_err(|_| "Source capture retention overflow")?,
                },
                snapshot: &selection.snapshot,
                entry: &selection.entries[0],
                context: CheckContextRevision {
                    grammar_edition: "conduitese-v1".into(),
                    catalog: "conduit/product-standard@1".into(),
                    kinds_fores: "conduit/product-standard@1".into(),
                    types_laws: "conduit/product-standard@1".into(),
                    backs: "empty@1".into(),
                },
                control: &control,
            },
            creche,
            Some(selection.caller_workspace_bytes),
            text_file,
            ambient,
        )?
    };
    let host = initialized.host;
    let caller = initialized
        .caller
        .ok_or("selected Source compiler lost original caller custody")?;
    // Product catalogs are immutable OnceLock stores. Their original charge
    // must survive Host/runtime destruction and failed later preparation alike.
    // Retain the first actual reservation until process exit, not merely while
    // this particular adoption/Host happens to exist.
    static CATALOG_CUSTODY: std::sync::OnceLock<
        conduit_std_host::body_execution::ImageMemoryRetention,
    > = std::sync::OnceLock::new();
    let shared_custody = CATALOG_CUSTODY.get_or_init(|| caller.retain_memory());
    host.verify_selected_image_memory_retention(shared_custody)?;
    Ok(PreparedFileSource {
        caller_workspace_bytes: selection.caller_workspace_bytes,
        host,
        source: initialized.source,
        caller,
    })
}
