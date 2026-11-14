//! Construct the actual installed Host and explicitly selected equipment before Boot publication.
use super::startup_trace::{Phase, Trace};
use super::{
    bounded_file_digest, current_body_id, fresh_identity, owner, read_installation,
    restrict_directory, running_target_id, write_json_atomic, RuntimeStatus,
    MAXIMUM_RELEASE_FILE_BYTES, RUNTIME_SCHEMA,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::{fs, path::Path};

#[cfg(test)]
pub(super) fn prepare_runtime(
    state_dir: &Path,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    prepare_traced_runtime(state_dir, &Trace::disabled())
}

pub(super) fn prepare_traced_runtime(
    state_dir: &Path,
    trace: &Trace,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    let thermostat = trace.run(Phase::RetainedScan, || retained_local_owner_scan(state_dir))?;
    prepare_runtime_with_composition(
        state_dir,
        None,
        thermostat
            .as_ref()
            .map(|(initial, maximum)| (initial, *maximum)),
        StdHostComposition::reference(),
        trace,
        #[cfg(all(target_os = "linux", feature = "native-display"))]
        None,
    )
}

/// Select the request-scoped Todo Back before the Host advertisement and
/// resource ledger are constructed. Retained equipment is composed only from
/// explicit selections before the Host is exposed.
pub(super) fn prepare_runtime_with_todo(
    state_dir: &Path,
    todo: Option<(&conduit_todo_plot::TodoState, u16)>,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    prepare_runtime_with_scans(state_dir, todo, None)
}

/// Select only the admitted domain-owned scan before Host advertisement.
pub(super) fn prepare_runtime_with_scans(
    state_dir: &Path,
    todo: Option<(&conduit_todo_plot::TodoState, u16)>,
    thermostat: Option<(&conduit_thermostat_plot::ThermostatState, u16)>,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    prepare_runtime_with_composition(
        state_dir,
        todo,
        thermostat,
        StdHostComposition::reference(),
        &Trace::disabled(),
        #[cfg(all(target_os = "linux", feature = "native-display"))]
        None,
    )
}

pub(super) fn prepare_presentation_runtime(
    state_dir: &Path,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    prepare_runtime_with_composition(
        state_dir,
        None,
        None,
        StdHostComposition::minimal().with_text(),
        &Trace::disabled(),
        #[cfg(all(target_os = "linux", feature = "native-display"))]
        None,
    )
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
pub(super) fn prepare_traced_runtime_with_scanout(
    state_dir: &Path,
    trace: &Trace,
    scanout: &mut conduit_linux_display::Scanout<'_>,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    if retained_local_owner_scan(state_dir)?.is_some() {
        return Err("selected scanout cannot preserve a scoped Thermostat Host".into());
    }
    prepare_runtime_with_composition(
        state_dir,
        None,
        None,
        StdHostComposition::reference(),
        trace,
        Some(scanout),
    )
}

/// A joined Host has a remote Body, not a locally owned retained Plot.
/// Keep owner-only state loading strict; it must not be a member Boot probe.
fn retained_local_owner_scan(
    state_dir: &Path,
) -> Result<Option<(conduit_thermostat_plot::ThermostatState, u16)>, String> {
    if read_installation(&state_dir.join("installation.json"))?
        .joined_body_state
        .is_some()
    {
        return Ok(None);
    }
    // A selected compiler Root must reserve its original caller envelope before
    // any retained Source parse. Do not cold-compile a scoped scan as a probe
    // before that IMAGE exists; actual complete workset checks run afterward.
    if super::source_revision::retained(state_dir)?.is_some() {
        return Ok(None);
    }
    owner::retained_thermostat_scan(state_dir)
}

fn prepare_runtime_with_composition(
    state_dir: &Path,
    todo: Option<(&conduit_todo_plot::TodoState, u16)>,
    thermostat: Option<(&conduit_thermostat_plot::ThermostatState, u16)>,
    composition: StdHostComposition,
    trace: &Trace,
    #[cfg(all(target_os = "linux", feature = "native-display"))] scanout: Option<
        &mut conduit_linux_display::Scanout<'_>,
    >,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    if todo.is_some() && thermostat.is_some() {
        return Err("installed Host accepts one scoped scan domain".into());
    }
    let installation = trace.run(Phase::Installation, || {
        read_installation(&state_dir.join("installation.json"))
    })?;
    let pending_seed = super::has_pending_seed(state_dir)?;
    let selected_image = installation
        .selected_host_configuration
        .as_ref()
        .and_then(|selection| selection.host_image());
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    if scanout.is_some() && selected_image.is_none() {
        return Err("initialized scanout has no selected Seed IMAGE".into());
    }
    let source_selection = super::source_revision::retained(state_dir)?;
    #[cfg(not(target_os = "linux"))]
    if source_selection.is_some() {
        return Err("selected bounded File compiler requires Linux".into());
    }
    if source_selection.is_some() && selected_image.is_none() {
        return Err("selected File compiler requires its installed IMAGE".into());
    }
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    let mut image_recognizer = if selected_image.is_some() && scanout.is_some() {
        installation
            .selected_host_configuration
            .as_ref()
            .ok_or("selected IMAGE lost Host source")?
            .prepare_recognizer()?
    } else {
        None
    };
    if selected_image.is_some() {
        if todo.is_some()
            || thermostat.is_some()
            || installation.selected_model.is_some()
            || installation.selected_speech.is_some()
            || installation.selected_todo_checkpoint.is_some()
        {
            return Err("selected timer IMAGE cannot preserve unrelated initialized equipment or scoped scans".into());
        }
        let selection = installation
            .selected_host_configuration
            .as_ref()
            .ok_or("selected IMAGE lost its source")?;
        #[cfg(all(target_os = "linux", feature = "native-display"))]
        if scanout.is_some() {
            selection.require_image_equipment_initialization(
                true,
                source_selection.is_some(),
                image_recognizer.is_some(),
            )?;
        } else {
            selection.require_prepared_image_initialization(false, source_selection.is_some())?;
        }
        #[cfg(not(all(target_os = "linux", feature = "native-display")))]
        selection.require_prepared_image_initialization(false, source_selection.is_some())?;
    }
    if (todo.is_some() || thermostat.is_some())
        && (installation.selected_model.is_some()
            || installation.selected_speech.is_some()
            || installation.selected_todo_checkpoint.is_some()
            || installation.selected_host_configuration.is_some())
    {
        return Err(if thermostat.is_some() {
            "installed Thermostat scan cannot preserve selected equipment in its scoped Host"
        } else {
            "installed Todo scan cannot preserve selected equipment in its scoped Host"
        }
        .into());
    }
    if installation.selected_todo_checkpoint.is_some()
        && installation.selected_host_configuration.is_some()
    {
        return Err(
            "scoped Todo checkpoint Host cannot initialize additional checked equipment".into(),
        );
    }
    let config = trace.run(Phase::BootIdentity, || {
        let boot_id = fresh_identity("boot/installed", &installation.host_id);
        Ok(StdHostConfig {
            host_id: HostId::from(installation.host_id.as_str()),
            boot_id: BootId::from(boot_id.as_str()),
            offer_generation: OfferGeneration(1),
        })
    })?;
    // Keep the actual initialized provider alive before constructing its slot.
    // A checked Base alone cannot publish this offer.
    let creche_factory = installation
        .selected_host_configuration
        .as_ref()
        .map(|selection| selection.prepare_creche())
        .transpose()?
        .flatten();
    if creche_factory.is_some()
        && (todo.is_some()
            || thermostat.is_some()
            || installation.selected_model.is_some()
            || installation.selected_todo_checkpoint.is_some())
    {
        return Err("selected Crèche provider cannot preserve this scoped Host composition".into());
    }
    #[cfg(target_os = "linux")]
    let mut source_custody = None;
    let mut host = trace.run(Phase::HostConstruction, || {
        Ok(if let Some(image) = selected_image {
            #[cfg(target_os = "linux")]
            if let Some(source_selection) = &source_selection {
                #[cfg(all(target_os = "linux", feature = "native-display"))]
                if scanout.is_some() {
                    return Err(
                        "selected File compiler cannot also initialize scanout in this composition"
                            .into(),
                    );
                }
                let prepared = super::source_revision::prepare(
                    source_selection,
                    &installation,
                    config,
                    creche_factory
                        .as_ref()
                        .map(|factory| factory.resource_offer()),
                )?;
                source_custody = Some((prepared.source, prepared.caller));
                return Ok(prepared.host);
            }
            #[cfg(all(target_os = "linux", feature = "native-display"))]
            if let Some(output) = scanout {
                let checked = installation
                    .selected_host_configuration
                    .as_ref()
                    .ok_or("scanout lost selected Source")?
                    .checked()?;
                let parameters =
                    conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(
                        &checked,
                    )?;
                let input = parameters
                    .input()
                    .is_some()
                    .then(|| {
                        output
                            .prepared_input()
                            .map_err(|error| format!("selected SDL input not prepared:{error:?}"))
                    })
                    .transpose()?;
                return StdHost::from_selected_timer_native_mask_with_equipment_image(
                    config,
                    image,
                    &conduit_workspace_make::catalog(),
                    &checked,
                    parameters,
                    output,
                    image_recognizer.take(),
                    input,
                );
            }
            if installation
                .selected_host_configuration
                .as_ref()
                .map(|selection| selection.has_cpu_initialization())
                .transpose()?
                .unwrap_or(false)
            {
                if creche_factory.is_some() {
                    return Err(
                        "CPU and Crèche require a combined checked initial construction".into(),
                    );
                }
                let checked = installation
                    .selected_host_configuration
                    .as_ref()
                    .ok_or("CPU lost selected Source")?
                    .checked()?;
                return StdHost::from_selected_timer_and_cpu_image(
                    config,
                    image,
                    &conduit_workspace_make::catalog(),
                    &checked,
                );
            }
            #[cfg(target_os = "linux")]
            if let Some(parameters) = installation
                .selected_host_configuration
                .as_ref()
                .map(|selection| selection.text_file_parameters())
                .transpose()?
                .flatten()
            {
                if creche_factory.is_some() {
                    return Err(
                        "immutable Text and Crèche require a combined checked initial construction"
                            .into(),
                    );
                }
                let _ = parameters;
                return Err("selected Text IMAGE requires independently owned directory and authority through original construction".into());
            }
            if let Some(factory) = &creche_factory {
                StdHost::from_selected_timer_and_creche_image(
                    config,
                    image,
                    &conduit_workspace_make::catalog(),
                    factory.resource_offer(),
                )?
            } else {
                StdHost::from_selected_timer_image(
                    config,
                    image,
                    &conduit_workspace_make::catalog(),
                )?
            }
        } else if let Some(factory) = &creche_factory {
            StdHost::new_with_creche_encounter(config, composition, factory.resource_offer())?
        } else if let Some((initial, maximum_items)) = todo {
            StdHost::new_for_todo_scan(config, initial, maximum_items)?
        } else if let Some((initial, maximum_items)) = thermostat {
            StdHost::new_for_thermostat_scan(config, initial, maximum_items)?
        } else if let Some(selection) = &installation.selected_todo_checkpoint {
            selection.validate()?;
            if let Some(model) = &installation.selected_model {
                StdHost::new_for_todo_checkpoint_with_local_model(
                    config,
                    selection.root(),
                    selection.content(),
                    Box::new(model.initialize()?),
                )?
            } else {
                StdHost::new_for_todo_checkpoint_once(
                    config,
                    selection.root(),
                    selection.content(),
                )?
            }
        } else if let Some(selection) = &installation.selected_model {
            StdHost::new_with_local_model(config, composition, Box::new(selection.initialize()?))?
        } else {
            StdHost::new_with_composition(config, composition)
        })
    })?;
    let selected_equipment = trace.run(Phase::EquipmentSelection, || {
        if selected_image.is_none() {
            if let Some(selection) = &installation.selected_host_configuration {
                selection.attach_to_prepared_host(&mut host, creche_factory.as_ref())?;
            }
        }
        Ok(if let Some(selection) = &installation.selected_speech {
            let artifact_dir = state_dir.join("spoken-artifacts");
            fs::create_dir_all(&artifact_dir)
                .map_err(|error| format!("create spoken artifact directory: {error}"))?;
            restrict_directory(&artifact_dir)?;
            // Retained capacity is selected before Boot advertisement; each
            // admitted Play derives a create-new destination from exact identity.
            selection.attach_to_fresh_host_with_artifact(&mut host, &artifact_dir)?
        } else {
            None
        })
    })?;
    let status = trace.run(Phase::RuntimeIdentity, || {
        Ok(RuntimeStatus {
            schema: RUNTIME_SCHEMA.into(),
            host_id: host.advertisement().host_id.as_str().into(),
            boot_id: host.advertisement().boot_id.as_str().into(),
            offer_generation: host.advertisement().offer_generation.0,
            process_id: std::process::id(),
            body_id: current_body_id(&installation).map(str::to_owned),
            release_bundle_sha256: installation.release_bundle_sha256.clone(),
        })
    })?;
    let executable = Path::new(&installation.product_executable);
    let image_content_digest = trace.run(Phase::ImageDigest, || {
        bounded_file_digest(executable, MAXIMUM_RELEASE_FILE_BYTES)
    })?;
    let mut runtime = trace.run(Phase::RuntimeState, || {
        Ok(crate::durable_host_control::DurableHostRuntime::new(
            running_target_id()?.into(),
            image_content_digest,
            host,
        )
        .with_selected_speech_equipment(selected_equipment)
        .with_prepared_creche(creche_factory.map(Into::into)))
    })?;
    #[cfg(target_os = "linux")]
    runtime.retain_source_preparation(source_custody);
    // Cold reviewed inventory and compiled contracts belong to preparation,
    // before clients can spend their finite control budget on an interaction.
    trace.run(Phase::Catalogs, crate::plot_source::standard_catalogs)?;
    if installation.body_state.is_none()
        && installation.joined_body_state.is_none()
        && !pending_seed
    {
        trace.run(Phase::BirthEncounter, || {
            runtime.prepare_birth_encounter(state_dir)
        })?;
    }
    trace.run(Phase::RuntimePublication, || {
        write_json_atomic(&state_dir.join("runtime.json"), &status)
    })?;
    Ok((status, runtime))
}
