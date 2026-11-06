//! Selected compiler installation, with Source acquisition inside funded preparation.
use super::{derive_image_definition, SourceCheckProviderIdentity};
use conduit_core::AuthorityGrant;
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_plot::{CheckContextRevision, SourceCheckNativeSchema};
use conduit_std_host::{
    body_execution::ImageForeInput,
    source_check::{
        process::OwnedSourceCheckInput, PreparedSourceChecker, SourceCheckPreparationBounds,
    },
    ExternalForeInput, ImmediateSourcePreparationBudget, RunControl, StdHost, StdHostConfig,
};
use std::fs::File;

/// Root-owned selection. Labels cannot open a file or authorize its contents.
/// The descriptor is consumed once; all Source bytes are captured after backing
/// admission. The schema and checked configuration remain borrowed throughout.
pub struct SourceCheckImageInstallation<'a> {
    pub config: StdHostConfig,
    pub image: &'a HostImage,
    pub checked: &'a CheckedHostConfiguration,
    pub catalog: &'a MakeCatalog,
    pub grant: AuthorityGrant,
    pub provider: SourceCheckProviderIdentity,
    pub schema: &'a SourceCheckNativeSchema,
    pub file: File,
    pub maximum_source_bytes: usize,
    pub owner: &'a str,
    pub generation: u64,
    pub entry: &'a str,
    pub context: CheckContextRevision,
    pub control: &'a RunControl,
}

/// Successful initialization and the exact encoded request from its one File
/// capture. Its backing remains charged to the IMAGE even if the Host drops
/// first. This request is descriptive metadata, not a Source grant. Other
/// caller-owned authoring metadata and routing need independent custody.
pub struct InitializedSourceCheckImage {
    pub host: StdHost,
    pub request: ImageForeInput,
}

/// Construct a Host only after selected Source capture and the actual bounded
/// compiler succeed. IMAGE/grant/provider correspondence and memory admission
/// use the existing Root constructor; failed preparation publishes no Host.
/// This does not adopt or execute the checked candidate.
///
/// # Safety
/// Root must independently own the readable File and authorize the exact Source
/// scope and Host grant. Its schema, configuration and caller
/// workspace need independent custody. The checked preparation ceiling must
/// cover complete parent/child preparation, and retirement must satisfy the
/// existing immediate-provider contract. Names and IMAGE intent grant nothing.
pub unsafe fn initialize_image(
    request: SourceCheckImageInstallation<'_>,
) -> Result<InitializedSourceCheckImage, String> {
    // SAFETY: this preserves the caller's exact independent obligations.
    unsafe { initialize_image_with_prepared_creche(request, None) }
}

/// Compose an already prepared Crèche encounter slot in the initial IMAGE
/// ledger alongside the bounded Source checker. The caller must retain the
/// matching actual Crèche factory for the lifetime of this Host; a resource
/// description alone does not supply an execution adapter or Source authority.
///
/// # Safety
/// All obligations of [`initialize_image`] apply, in addition to the actual
/// prepared Crèche factory's custody and exact resource correspondence.
pub unsafe fn initialize_image_with_prepared_creche(
    request: SourceCheckImageInstallation<'_>,
    creche_resource: Option<conduit_core::ResourceOffer>,
) -> Result<InitializedSourceCheckImage, String> {
    let selected = conduit_workspace_make::CheckedSourceCheckInitialization::from_configuration(
        request.checked,
    )?;
    let bounds = SourceCheckPreparationBounds {
        request_bytes: selected
            .request_bytes
            .try_into()
            .map_err(|_| "request bound overflow")?,
        report_bytes: selected
            .reply_bytes
            .try_into()
            .map_err(|_| "reply bound overflow")?,
        retained_bytes: selected
            .retained_bytes
            .try_into()
            .map_err(|_| "retained bound overflow")?,
    };
    let definition = derive_image_definition(
        &request.config,
        request.image,
        request.checked,
        request.catalog,
        request.schema,
        request.grant,
        request.provider,
    )?;
    let budget = ImmediateSourcePreparationBudget {
        preparation_bytes: selected
            .preparation_bytes
            .try_into()
            .map_err(|_| "preparation bound overflow")?,
        retained_bytes: selected
            .retained_bytes
            .try_into()
            .map_err(|_| "retained bound overflow")?,
    };
    let mut captured_request = None;
    // SAFETY: caller retains the independent authority/workspace obligations;
    // this factory supplies the actual selected, bounded process implementation.
    let mut host = unsafe {
        StdHost::from_selected_timer_and_immediate_source_and_creche_image(
            request.config,
            request.image,
            request.catalog,
            request.checked,
            definition,
            budget,
            creche_resource,
            || {
                if request.control.stop_requested() {
                    return Err("Source-check preparation cancelled".into());
                }
                let mut worker = super::from_configuration(request.checked)?;
                let source = OwnedSourceCheckInput::from_file(
                    request.file,
                    request.maximum_source_bytes,
                    request.owner,
                    request.generation,
                    request.entry,
                    request.context,
                )
                .map_err(|e| format!("selected Source capture: {e:?}"))?;
                let output = worker
                    .prepare_selected_source(&source, request.control)
                    .map_err(|e| format!("selected Source compiler: {e:?}"))?;
                let prepared = PreparedSourceChecker::from_process_outputs(
                    request.schema,
                    &[output],
                    bounds,
                    request.control,
                )?;
                // Encode under the still-reserved complete preparation ceiling.
                // Do not let a second uncharged typed request escape that scope.
                let bytes = request
                    .schema
                    .encode_request(source.request(), bounds.request_bytes as usize)
                    .map_err(|error| format!("selected Source request: {error:?}"))?;
                captured_request = Some(ExternalForeInput {
                    front_port_id: "request".into(),
                    track: conduit_core::ConnectionTrack::Payload,
                    bytes,
                });
                Ok(Box::new(prepared.into_provider()))
            },
        )
    }?;
    // The constructor invokes the successful factory exactly once. Neither a
    // failed Host nor partial selection metadata escapes an earlier refusal.
    let request = host.retain_selected_image_fore_input(
        captured_request.ok_or("initialized Source checker lacks its capture")?,
    )?;
    Ok(InitializedSourceCheckImage { host, request })
}
