//! Selected compiler installation, with Source acquisition inside funded preparation.
use super::{derive_image_definition, SourceCheckProviderIdentity};
use conduit_core::AuthorityGrant;
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_plot::{CheckContextRevision, SourceCheckNativeSchema, SourceSnapshotRevision};
use conduit_std_host::{
    body_execution::{ImageForeInput, ImagePreparedValue},
    source_check::{
        process::OwnedSourceCheckInput, PreparedSourceChecker, SourceCheckPreparationBounds,
    },
    ExternalForeInput, ImmediateSourcePreparationBudget, RunControl, StdHost, StdHostConfig,
};
use std::fs::File;

/// Root-owned selection. Labels cannot open a file or authorize its contents.
/// The descriptor is consumed once; all Source bytes are captured after backing
/// admission. The schema is constructed under staged IMAGE custody; checked
/// configuration and catalog retain independently admitted caller custody.
pub struct SourceCheckImageInstallation<'a> {
    pub config: StdHostConfig,
    pub image: &'a HostImage,
    pub checked: &'a CheckedHostConfiguration,
    pub catalog: &'a MakeCatalog,
    pub grant: AuthorityGrant,
    pub provider: SourceCheckProviderIdentity,
    pub file: File,
    pub maximum_source_bytes: usize,
    /// Independently qualified capture peak and complete immutable retained
    /// backing, separately charged while metadata/provider preparation overlap.
    /// This is a ceiling on the existing IMAGE counter, never an added grant.
    pub capture_budget: ImmediateSourcePreparationBudget,
    /// Exact independently reviewed snapshot; these labels confer no read authority.
    pub snapshot: &'a SourceSnapshotRevision,
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
    /// The actual immutable File capture, retaining its original IMAGE charge.
    /// A report or copied snapshot is never substituted for these bytes.
    pub source: ImagePreparedValue<OwnedSourceCheckInput>,
    /// Optional complete caller envelope reserved before any File capture or
    /// compiler launch on the same original counter.
    pub caller: Option<ImagePreparedValue<()>>,
}

/// Construct a Host only after selected Source capture and the actual bounded
/// compiler succeed. IMAGE/grant/provider correspondence and memory admission
/// use the existing Root constructor; failed preparation publishes no Host.
/// This does not adopt or execute the checked candidate.
///
/// # Safety
/// Root must independently own the readable File and authorize the exact Source
/// scope and Host grant. Its checked configuration/catalog and caller
/// workspace need independent custody. The capture budget must qualify all
/// synchronous File acquisition/hash/metadata peak and every retained capacity;
/// caller-owned captures remain funded until transferred. The checked preparation
/// ceiling must cover complete parent/child preparation, and retirement must satisfy the
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
    // SAFETY: preserve the caller's independent external workspace custody.
    unsafe { initialize_image_with_caller_workspace(request, creche_resource, None, None) }
}

/// Additionally reserve a separately qualified caller envelope before Source
/// capture and compiler launch. Its full charge survives all subsequent overlap.
///
/// # Safety
/// All obligations of [`initialize_image_with_prepared_creche`] apply. When
/// supplied, the caller envelope must independently cover every parent catalog,
/// compiler, owner/planner/journal overlap and retained shared store it funds;
/// it supplies neither Source access nor adoption authority. Optional Text
/// storage enters the same cold constructor with its real directory and
/// independent current operation grant; none is inferred from advertised needs.
pub unsafe fn initialize_image_with_caller_workspace(
    request: SourceCheckImageInstallation<'_>,
    creche_resource: Option<conduit_core::ResourceOffer>,
    caller_workspace_bytes: Option<usize>,
    text_file: Option<conduit_std_host::TextFileInitialization>,
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
    let capture = &mut captured_request;
    let mut captured_source = None;
    let source_capture = &mut captured_source;
    let mut captured_caller = None;
    let caller_capture = &mut captured_caller;
    // SAFETY: caller retains the independent authority/workspace obligations;
    // this factory supplies the actual selected, bounded process implementation.
    let mut host = unsafe {
        StdHost::from_selected_timer_and_source_factory_image(
            request.config,
            request.image,
            request.catalog,
            request.checked,
            budget,
            creche_resource,
            text_file,
            |config, root| {
                let caller = caller_workspace_bytes
                    .map(|bytes| root.prepare_value(bytes, bytes, || Ok(())))
                    .transpose()?;
                let schema = SourceCheckNativeSchema::prepare()
                    .map_err(|error| format!("selected Source schema: {error:?}"))?;
                let definition = derive_image_definition(
                    config,
                    request.image,
                    request.checked,
                    request.catalog,
                    &schema,
                    request.grant,
                    request.provider,
                )?;
                // Acquire only after reserving the separate capture envelope on
                // this original counter. Metadata and later native preparation
                // retain their own distinct overlapping charges.
                let source = root.prepare_value(
                    request.capture_budget.preparation_bytes as usize,
                    request.capture_budget.retained_bytes as usize,
                    || {
                        if request.control.stop_requested() {
                            return Err("Source-check preparation cancelled before capture".into());
                        }
                        let source = OwnedSourceCheckInput::from_file(
                            request.file,
                            request.maximum_source_bytes,
                            &request.snapshot.owner,
                            request.snapshot.generation,
                            request.entry,
                            request.context,
                        )
                        .map_err(|e| format!("selected Source capture: {e:?}"))?;
                        let bytes = source
                            .owned_backing_bytes()
                            .ok_or("Source capture backing overflow")?;
                        if bytes > request.capture_budget.retained_bytes as usize {
                            return Err("Source capture exceeds qualified retained backing".into());
                        }
                        if &source.request().snapshot != request.snapshot {
                            return Err("selected Source changed since owner review".into());
                        }
                        Ok(source)
                    },
                )?;
                Ok((definition, budget, move || {
                    if request.control.stop_requested() {
                        return Err("Source-check preparation cancelled".into());
                    }
                    let mut worker = super::from_configuration(request.checked)?;
                    let output = worker
                        .prepare_selected_source(source.value(), request.control)
                        .map_err(|e| format!("selected Source compiler: {e:?}"))?;
                    let prepared = PreparedSourceChecker::from_process_outputs(
                        &schema,
                        &[output],
                        bounds,
                        request.control,
                    )?;
                    // Encode under the still-reserved complete preparation ceiling.
                    // Do not let a second uncharged typed request escape that scope.
                    let bytes = schema
                        .encode_request(source.value().request(), bounds.request_bytes as usize)
                        .map_err(|error| format!("selected Source request: {error:?}"))?;
                    *capture = Some(ExternalForeInput {
                        front_port_id: "request".into(),
                        track: conduit_core::ConnectionTrack::Payload,
                        bytes,
                    });
                    *source_capture = Some(source);
                    *caller_capture = caller;
                    Ok(Box::new(prepared.into_provider())
                        as Box<dyn conduit_std_host::ImmediateNativeSource>)
                }))
            },
        )
    }?;
    // The constructor invokes the successful factory exactly once. Neither a
    // failed Host nor partial selection metadata escapes an earlier refusal.
    let request = host.retain_selected_image_fore_input(
        captured_request.ok_or("initialized Source checker lacks its capture")?,
    )?;
    let source = captured_source.ok_or("initialized Source checker lacks its actual capture")?;
    host.verify_selected_image_prepared_value(&source)?;
    Ok(InitializedSourceCheckImage {
        host,
        request,
        source,
        caller: captured_caller,
    })
}
