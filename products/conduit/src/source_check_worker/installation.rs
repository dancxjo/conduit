//! Selected compiler installation, with Source acquisition inside funded preparation.
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_plot::{CheckContextRevision, SourceCheckNativeSchema};
use conduit_std_host::{
    source_check::{
        process::OwnedSourceCheckInput, PreparedSourceChecker, SourceCheckPreparationBounds,
    },
    ImmediateSourcePreparationBudget, NativeSourceDefinition, RunControl, StdHost, StdHostConfig,
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
    pub definition: NativeSourceDefinition,
    pub schema: &'a SourceCheckNativeSchema,
    pub file: File,
    pub maximum_source_bytes: usize,
    pub owner: &'a str,
    pub generation: u64,
    pub entry: &'a str,
    pub context: CheckContextRevision,
    pub control: &'a RunControl,
}

/// Construct a Host only after selected Source capture and the actual bounded
/// compiler succeed. IMAGE/grant/provider correspondence and memory admission
/// use the existing Root constructor; failed preparation publishes no Host.
/// This does not adopt or execute the checked candidate.
///
/// # Safety
/// Root must independently own the readable File and authorize the exact Source
/// scope and Host grant. Its schema, configuration, definition and caller
/// workspace need independent custody. The checked preparation ceiling must
/// cover complete parent/child preparation, and retirement must satisfy the
/// existing immediate-provider contract. Names and IMAGE intent grant nothing.
pub unsafe fn initialize_image(
    request: SourceCheckImageInstallation<'_>,
) -> Result<StdHost, String> {
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
    let kind = PreparedSourceChecker::kind_for_bounds(request.schema, bounds)?;
    if request.definition.kind != kind
        || request.definition.input_bytes != bounds.request_bytes
        || request.definition.output_bytes != bounds.report_bytes
        || request.definition.resource_bytes != selected.retained_bytes
    {
        return Err("Source-check definition differs from checked compiler envelopes".into());
    }
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
    // SAFETY: caller retains the independent authority/workspace obligations;
    // this factory supplies the actual selected, bounded process implementation.
    unsafe {
        StdHost::from_selected_timer_and_immediate_source_image(
            request.config,
            request.image,
            request.catalog,
            request.checked,
            request.definition,
            budget,
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
                Ok(Box::new(prepared.into_provider()))
            },
        )
    }
}
