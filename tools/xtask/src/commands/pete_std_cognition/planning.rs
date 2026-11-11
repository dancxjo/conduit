//! Preparation of identity, continuity, and exact ordinary Plot realization.
use super::*;
pub(super) fn context(
    authoring: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<BodyConversationContext, String> {
    let body = Body::born(
        authoring.expanded.source_document_id.clone(),
        authoring.expanded.checked_plot_id.clone(),
        1,
        format!("sign/pete/born/{}", std::process::id()).into(),
    )
    .map_err(|error| format!("birth Pete: {error:?}"))?;
    let (_, wake) = body
        .wake(2, format!("sign/pete/wake/{}", std::process::id()).into())
        .map_err(|error| format!("wake Pete: {error:?}"))?;
    Ok(BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: "Pete".into(),
        body_id: wake.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence: 2,
        basis: BodyConversationContextBasis {
            body_id: wake.body_id,
            wake_id: wake.wake_id,
            wake_sequence: 2,
            revision: 1,
        },
        hosts: vec![],
        active_plots: vec![authoring.expanded.name.clone()],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    })
}

pub(super) fn checkpoint_context(
    root: &std::path::Path,
    authoring: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<(BodyConversationContext, std::fs::File), String> {
    use std::io::{Read, Write};

    let io_error = |error| format!("Pete checkpoint directory {}: {error}", root.display());
    std::fs::create_dir_all(root).map_err(io_error)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("operator.lock"))
        .map_err(io_error)?;
    lock.try_lock()
        .map_err(|error| format!("lock Pete checkpoint directory {}: {error}", root.display()))?;
    let path = root.join("context.json");
    let context = match std::fs::File::open(&path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(32_769)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() > 32_768 {
                return Err("Pete checkpoint context exceeds its byte bound".into());
            }
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("decode Pete checkpoint context: {error}"))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let context = context(authoring)?;
            let candidate = root.join("context.pending");
            let bytes = serde_json::to_vec(&context).map_err(|error| error.to_string())?;
            let mut file = std::fs::File::create(&candidate).map_err(io_error)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(io_error)?;
            std::fs::rename(candidate, path).map_err(io_error)?;
            std::fs::File::open(root)
                .and_then(|directory| directory.sync_all())
                .map_err(io_error)?;
            context
        }
        Err(error) => return Err(io_error(error)),
    };
    Ok((context, lock))
}

pub(super) fn authoring(
    checkpoint: Option<&[u8]>,
) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
    authoring_source(checkpoint, false)
}
pub(super) fn authoring_cadenced(
    checkpoint: Option<&[u8]>,
) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
    authoring_source(checkpoint, true)
}
fn authoring_source(
    checkpoint: Option<&[u8]>,
    cadenced: bool,
) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile)?;
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile)?;
    conduit_chat::install_episode_loop_catalog(&mut startup, &mut profile)?;
    conduit_pete::install_pete_cognition_catalog(&mut startup, &mut profile)?;
    if let Some(checkpoint) = checkpoint {
        conduit_chat::install_episode_resumable_memory_catalog(
            checkpoint,
            &mut startup,
            &mut profile,
        )?;
    }
    if cadenced {
        let (value, trigger) = cadence_contracts()?;
        conduit_semantic_catalog::install_flow_coalesce_finite_kind(
            &value,
            &mut startup,
            &mut profile,
        )?;
        conduit_semantic_catalog::install_current_sample_finite_kind(
            &value,
            &trigger,
            &mut startup,
            &mut profile,
        )?;
    }
    let (source, entry) = match (cadenced, checkpoint.is_some()) {
        (true, false) => (
            include_str!("../../../../../plots/pete-cognition-cadenced/main.conduit"),
            "pete-cognition",
        ),
        (true, true) => (
            include_str!("../../../../../plots/pete-cognition-cadenced/resumable.conduit"),
            "pete-cognition",
        ),
        (false, false) => (SOURCE, "pete-cognition"),
        (false, true) => (RESUMABLE_SOURCE, "pete-cognition-resumable"),
    };
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check Pete Plot: {error:?}"))?;
    conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &profile)
        .map_err(|error| format!("expand Pete Plot: {error:?}"))
}

pub(super) fn plan(
    host: &StdHost,
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    resumable: bool,
) -> Result<PlanFragment, String> {
    plan_with_mode(host, authoring, resumable, false)
}
pub(super) fn plan_cadenced(
    host: &mut StdHost,
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    resumable: bool,
) -> Result<PlanFragment, String> {
    let (value, trigger) = cadence_contracts()?;
    host.install_finite_current_sampling(&value, &trigger)?;
    for gear in &authoring.expanded.gears {
        if gear.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
            let conduit_core::ConfigurationValue::Text(encoded) = &gear
                .configuration
                .iter()
                .find(|entry| entry.key == "program")
                .ok_or("missing checked filter program")?
                .value
            else {
                return Err("invalid checked filter program".into());
            };
            let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                .map_err(|error| format!("decode checked filter: {error:?}"))?;
            host.install_pure_filter(&program, gear.inputs[0].temporal)?;
        }
    }
    plan_with_mode(host, authoring, resumable, true)
}
fn cadence_contracts() -> Result<
    (
        conduit_core::CheckedValueContract,
        conduit_core::CheckedValueContract,
    ),
    String,
> {
    let value = conduit_core::CheckedValueContract::new(
        conduit_chat::episode_experience_type()
            .profile()
            .map_err(|e| format!("experience profile: {e:?}"))?
            .value_kind()
            .clone(),
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        vec![],
    )
    .map_err(|e| format!("experience envelope: {e:?}"))?;
    let trigger =
        conduit_core::CheckedValueContract::new(conduit_core::kind_id("value/u64"), 8, vec![])
            .map_err(|e| format!("trigger envelope: {e:?}"))?;
    Ok((value, trigger))
}
fn plan_with_mode(
    host: &StdHost,
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    resumable: bool,
    cadenced: bool,
) -> Result<PlanFragment, String> {
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| format!("place Pete Plot: {error:?}"))?;
    let boundaries = [
        ("context", PortDirection::Input, 32_768),
        (
            "frames",
            PortDirection::Input,
            conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        ),
        ("episodes", PortDirection::Output, 32_768),
    ]
    .into_iter()
    .chain(cadenced.then_some(("reflections", PortDirection::Input, 8)))
    .chain(cadenced.then_some((
        "reactions",
        PortDirection::Output,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
    )))
    .chain(cadenced.then_some((
        "attention",
        PortDirection::Output,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
    )))
    .chain(resumable.then_some((
        "checkpoints",
        PortDirection::Output,
        conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32,
    )))
    .map(|(name, direction, byte_capacity)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity,
            },
        )
    })
    .collect();
    let offer = |gear: &conduit_core::GearId| {
        let choice = placements.by_gear.get(gear).expect("selected Gear");
        hosts
            .iter()
            .find(|host| host.host_id == choice.host_id)
            .unwrap()
            .capabilities
            .iter()
            .find(|cap| cap.capability_id == choice.capability_id)
            .unwrap()
    };
    let connection_limits = authoring
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let source = authoring
                .expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == cord.source_gear_id)
                .unwrap();
            let maximum =
                if source.kind_id.as_str() == conduit_chat::EPISODE_RETRIEVAL_PRODUCER_KIND {
                    match cord.source_port_id.as_str() {
                        "capture" => conduit_chat::MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES as u32,
                        "window" => conduit_ai::TEMPORAL_EVIDENCE_FLOW_MAX_BYTES as u32,
                        _ => 16384,
                    }
                } else {
                    16384
                };
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: maximum
                        .min(offer(&cord.source_gear_id).limits.max_queue_bytes)
                        .min(offer(&cord.sink_gear_id).limits.max_queue_bytes),
                },
            )
        })
        .collect();
    let mut plan = plan_expanded_authoring_with_connection_limits(
        authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 16_384,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
        &boundaries,
    )
    .map_err(|error| format!("plan Pete Plot: {error:?}"))?;
    if plan.fragments.len() != 1 {
        return Err("Pete cognition requires one local fragment".into());
    }
    plan.fragments
        .pop()
        .ok_or_else(|| "Pete Plan lost its fragment".into())
}
