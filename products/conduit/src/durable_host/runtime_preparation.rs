//! Construct the actual installed Host and explicitly selected equipment before Boot publication.
use super::{
    bounded_read, current_body_id, digest, fresh_identity, owner, read_installation,
    restrict_directory, running_target_id, write_json_atomic, RuntimeStatus,
    MAXIMUM_RELEASE_FILE_BYTES, RUNTIME_SCHEMA,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::{fs, path::Path};

pub(super) fn prepare_runtime(
    state_dir: &Path,
) -> Result<
    (
        RuntimeStatus,
        crate::durable_host_control::DurableHostRuntime,
    ),
    String,
> {
    let thermostat = owner::retained_thermostat_scan(state_dir)?;
    prepare_runtime_with_scans(
        state_dir,
        None,
        thermostat
            .as_ref()
            .map(|(initial, maximum)| (initial, *maximum)),
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
    prepare_runtime_with_composition(state_dir, todo, thermostat, StdHostComposition::reference())
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
    )
}

fn prepare_runtime_with_composition(
    state_dir: &Path,
    todo: Option<(&conduit_todo_plot::TodoState, u16)>,
    thermostat: Option<(&conduit_thermostat_plot::ThermostatState, u16)>,
    composition: StdHostComposition,
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
    let installation = read_installation(&state_dir.join("installation.json"))?;
    if (todo.is_some() || thermostat.is_some())
        && (installation.selected_model.is_some()
            || installation.selected_speech.is_some()
            || installation.selected_todo_checkpoint.is_some())
    {
        return Err(if thermostat.is_some() {
            "installed Thermostat scan cannot preserve selected equipment in its scoped Host"
        } else {
            "installed Todo scan cannot preserve selected equipment in its scoped Host"
        }
        .into());
    }
    let boot_id = fresh_identity("boot/installed", &installation.host_id);
    let config = StdHostConfig {
        host_id: HostId::from(installation.host_id.as_str()),
        boot_id: BootId::from(boot_id.as_str()),
        offer_generation: OfferGeneration(1),
    };
    let mut host = if let Some((initial, maximum_items)) = todo {
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
            StdHost::new_for_todo_checkpoint_once(config, selection.root(), selection.content())?
        }
    } else if let Some(selection) = &installation.selected_model {
        StdHost::new_with_local_model(config, composition, Box::new(selection.initialize()?))?
    } else {
        StdHost::new_with_composition(config, composition)
    };
    let selected_equipment = if let Some(selection) = &installation.selected_speech {
        let artifact_dir = state_dir.join("spoken-artifacts");
        fs::create_dir_all(&artifact_dir)
            .map_err(|error| format!("create spoken artifact directory: {error}"))?;
        restrict_directory(&artifact_dir)?;
        // Retained capacity is selected before Boot advertisement; each
        // admitted Play derives a create-new destination from exact identity.
        selection.attach_to_fresh_host_with_artifact(&mut host, &artifact_dir)?
    } else {
        None
    };
    let status = RuntimeStatus {
        schema: RUNTIME_SCHEMA.into(),
        host_id: host.advertisement().host_id.as_str().into(),
        boot_id: host.advertisement().boot_id.as_str().into(),
        offer_generation: host.advertisement().offer_generation.0,
        process_id: std::process::id(),
        body_id: current_body_id(&installation).map(str::to_owned),
        release_bundle_sha256: installation.release_bundle_sha256.clone(),
    };
    let executable = Path::new(&installation.product_executable);
    let image_content_digest = digest(&bounded_read(executable, MAXIMUM_RELEASE_FILE_BYTES)?);
    let mut runtime = crate::durable_host_control::DurableHostRuntime::new(
        running_target_id()?.into(),
        image_content_digest,
        host,
    )
    .with_selected_speech_equipment(selected_equipment);
    // Cold reviewed inventory and compiled contracts belong to preparation,
    // before clients can spend their finite control budget on an interaction.
    crate::plot_source::prepare_standard_catalogs()?;
    if installation.body_state.is_none() && installation.joined_body_state.is_none() {
        runtime.prepare_birth_encounter(state_dir)?;
    }
    write_json_atomic(&state_dir.join("runtime.json"), &status)?;
    Ok((status, runtime))
}
