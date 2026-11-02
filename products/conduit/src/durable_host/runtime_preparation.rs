//! Construct the actual installed Host and explicitly selected equipment before Boot publication.
use super::startup_trace::{Phase, Trace};
use super::{
    bounded_read, current_body_id, digest, fresh_identity, owner, read_installation,
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
    let thermostat = trace.run(Phase::RetainedScan, || {
        owner::retained_thermostat_scan(state_dir)
    })?;
    prepare_runtime_with_composition(
        state_dir,
        None,
        thermostat
            .as_ref()
            .map(|(initial, maximum)| (initial, *maximum)),
        StdHostComposition::reference(),
        trace,
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
    )
}

fn prepare_runtime_with_composition(
    state_dir: &Path,
    todo: Option<(&conduit_todo_plot::TodoState, u16)>,
    thermostat: Option<(&conduit_thermostat_plot::ThermostatState, u16)>,
    composition: StdHostComposition,
    trace: &Trace,
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
    let mut host = trace.run(Phase::HostConstruction, || {
        Ok(if let Some(factory) = &creche_factory {
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
        if let Some(selection) = &installation.selected_host_configuration {
            selection.attach_to_prepared_host(&mut host, creche_factory.as_ref())?;
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
        Ok(digest(&bounded_read(
            executable,
            MAXIMUM_RELEASE_FILE_BYTES,
        )?))
    })?;
    let mut runtime = trace.run(Phase::RuntimeState, || {
        Ok(crate::durable_host_control::DurableHostRuntime::new(
            running_target_id()?.into(),
            image_content_digest,
            host,
        )
        .with_selected_speech_equipment(selected_equipment)
        .with_prepared_creche(creche_factory))
    })?;
    // Cold reviewed inventory and compiled contracts belong to preparation,
    // before clients can spend their finite control budget on an interaction.
    trace.run(Phase::Catalogs, crate::plot_source::standard_catalogs)?;
    if installation.body_state.is_none()
        && installation.joined_body_state.is_none()
        && !super::has_pending_seed(state_dir)?
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
