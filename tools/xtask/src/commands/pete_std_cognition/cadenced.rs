//! Finite operator-selected event/trigger ingress; ordinary Gears own cognition.
use super::*;

pub(super) fn validate_positions(positions: &[usize], events: usize) -> Result<(), String> {
    if positions.is_empty()
        || positions.iter().any(|&index| index == 0 || index > events)
        || positions.windows(2).any(|pair| pair[0] >= pair[1])
        || positions.last() != Some(&events)
    {
        return Err("reflection positions must increase strictly within the utterance list and include its final position".into());
    }
    Ok(())
}
pub(super) fn run(args: StdCognitionArgs) -> Result<(), String> {
    validate_positions(&args.reflection_after, args.utterance.len())?;
    let mut provider = args
        .serial_path
        .map(|device_path| {
            StdCreateUartBase::open(StdCreateUartObservation {
                base_id: CREATE_SOURCE.into(),
                device_path,
                profile: UartProfile::CREATE_OI,
                maximum_write_wait_ms: 250,
            })
            .map_err(|error| format!("open Create serial device: {error:?}"))
        })
        .transpose()?;
    let model = OllamaDiscovery::discover_at("http://127.0.0.1:11434", "gemma3:latest")
        .map_err(|error| format!("discover Ollama: {error:?}"))?
        .initialize(6144, vec![conduit_ai::LocalModelKindProfile::GenerateFlow])
        .map_err(|error| format!("initialize Ollama: {error:?}"))?;
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::prepare_pete_experience_input_bytes,
    )?;
    let initial = args
        .checkpoint_dir
        .as_ref()
        .map(|_| conduit_chat::initial_episode_memory());
    let initial_authoring = planning::authoring_cadenced(initial.as_deref())?;
    let (context, journal, _lock) = match args.checkpoint_dir {
        Some(root) => {
            let (context, lock) = checkpoint_context(&root, &initial_authoring)?;
            let journal = EpisodeCheckpointResidence::open(&root, &context)?;
            (context, Some(journal), Some(lock))
        }
        None => (context(&initial_authoring)?, None, None),
    };
    let checkpoint = journal
        .as_ref()
        .map(EpisodeCheckpointResidence::recover_seed)
        .transpose()?;
    let completed = checkpoint
        .as_deref()
        .map(conduit_chat::decode_episode_checkpoint)
        .transpose()
        .map_err(|error| format!("read Pete episode count: {error:?}"))?
        .map_or(0, |memory| *memory.completed());
    let expected = completed
        .checked_add(args.reflection_after.len() as u64)
        .ok_or("Pete episode count exhausted")?;
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("source session clock: {error}"))?
        .as_nanos();
    // Source Signs remain distinct when several frames share one episode count.
    let namespace = format!("{epoch:020x}");
    let config = StdHostConfig {
        host_id: "host/pete-cognition".into(),
        boot_id: format!("boot/pete-cognition/{}-{namespace}", std::process::id()).into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let mut host = match checkpoint.as_deref() {
        Some(checkpoint) => StdHost::new_with_local_model_frame_and_episode_checkpoint(
            config,
            StdHostComposition::minimal(),
            Box::new(model),
            back,
            checkpoint,
        ),
        None => StdHost::new_with_local_model_and_frame_back(
            config,
            StdHostComposition::minimal(),
            Box::new(model),
            back,
        ),
    }?;
    let authoring = match checkpoint.as_deref() {
        Some(checkpoint) => planning::authoring_cadenced(Some(checkpoint))?,
        None => initial_authoring,
    };
    planning::install_play_states(&mut host, &authoring)?;
    let fragment = planning::plan_cadenced(&mut host, &authoring, checkpoint.is_some())?;
    let mut inputs = Vec::with_capacity(1 + args.utterance.len() + args.reflection_after.len());
    inputs.push(ExternalForeInput {
        front_port_id: "context".into(),
        track: ConnectionTrack::Payload,
        bytes: conduit_chat::encode_body_conversation_context(&context)
            .map_err(|error| format!("encode Pete context: {error:?}"))?,
    });
    let mut reflections = args.reflection_after.iter().copied().peekable();
    let mut trigger = 0u64;
    for (index, utterance) in args.utterance.into_iter().enumerate() {
        let frame = frame_with_namespace(
            provider.as_mut(),
            index as u64 + 1,
            utterance,
            args.retain,
            &namespace,
        )?;
        inputs.push(ExternalForeInput {
            front_port_id: "frames".into(),
            track: ConnectionTrack::Payload,
            bytes: frame
                .encode()
                .map_err(|error| format!("encode Pete frame: {error:?}"))?,
        });
        if reflections.peek() == Some(&(index + 1)) {
            reflections.next();
            trigger += 1;
            inputs.push(ExternalForeInput {
                front_port_id: "reflections".into(),
                track: ConnectionTrack::Payload,
                bytes: trigger.to_le_bytes().to_vec(),
            });
        }
    }
    let archive = args
        .episodes_out
        .map(|path| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| format!("open Pete episode archive {}: {error}", path.display()))
        })
        .transpose()?;
    let mut output = CadencedOutput {
        episodes: EpisodeOutput::new(None, archive, journal, completed),
        reactions: 0,
        attention: 0,
    };
    let mut control = RunControl::default();
    if args.replay_out.is_some() {
        control = control.with_debug_body(context.body_id.clone());
    }
    let report = host.run_external_plot_sequence_controlled_to(
        fragment.clone(),
        &inputs,
        &mut output,
        &mut std::io::stderr(),
        &mut ThreadTimer,
        &control,
    )?;
    if !matches!(
        report.observations.last().map(|event| &event.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        })
    ) {
        return Err("Pete cadenced Play did not complete".into());
    }
    if output.episodes.acknowledged_episode() != expected {
        return Err(format!(
            "Pete acknowledged {} episodes, expected {expected}",
            output.episodes.acknowledged_episode()
        ));
    }
    eprintln!(
        "Pete cadence: {} reactions, {} attention updates, {} reflections.",
        output.reactions, output.attention, trigger
    );
    if let Some(path) = args.replay_out {
        use std::io::Write;
        let snapshot =
            super::super::pete_replay::snapshot(&fragment, &authoring, &context, &report)?;
        let bytes = snapshot.encode().map_err(|error| error.to_string())?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&bytes))
            .map_err(|error| format!("write Patchbay replay {}: {error}", path.display()))?;
    }
    Ok(())
}
struct CadencedOutput {
    episodes: EpisodeOutput,
    reactions: usize,
    attention: usize,
}
impl ExternalForeOutputAdapter for CadencedOutput {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        match delivery.front_port_id.as_str() {
            "reactions" => {
                self.reactions += 1;
                Ok(())
            }
            "attention" => {
                self.attention += 1;
                Ok(())
            }
            _ => self.episodes.deliver(delivery),
        }
    }
}
