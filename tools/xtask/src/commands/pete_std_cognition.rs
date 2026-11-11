//! Operator entrance for the ordinary Pete cognition Plot.

use clap::Args;
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{BaseImplementationId, ConnectionTrack, PlanFragment, PortDirection};
use conduit_pete::{
    CreateCognitionCaptureError, PeteCognitionBatteryInput, PeteCognitionFrame, PeteCognitionSign,
    PeteCognitionUtterance, UartProfile, capture_pete_cognition_battery,
};
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, ExternalForeInput,
    ExternalForeOutputAdapter, HostedFrameBack, RunControl, StdHost, StdHostComposition,
    StdHostConfig, ThreadTimer,
    episode_checkpoint::EpisodeCheckpointResidence,
    hosted_local_model::OllamaDiscovery,
    std_create_uart::{StdCreateUartBase, StdCreateUartObservation, monotonic_millis},
};
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};

#[derive(Args, Debug)]
pub(super) struct StdCognitionArgs {
    /// Explicit Create serial device. Omit for a session with unknown battery state.
    #[arg(long)]
    serial_path: Option<PathBuf>,
    /// One human utterance per episode, in order.
    #[arg(long, required = true)]
    utterance: Vec<String>,
    /// Retain the supplied utterances for recall within this Play.
    #[arg(long)]
    retain: bool,
    /// Write a new browser Patchbay replay for the completed Play.
    #[arg(long)]
    replay_out: Option<PathBuf>,
    /// Write each delivered typed episode to a new JSON Lines file.
    #[arg(long)]
    episodes_out: Option<PathBuf>,
    /// Resume the same Body and Wake from this checkpoint directory.
    #[arg(long)]
    checkpoint_dir: Option<PathBuf>,
}

const SOURCE: &str = include_str!("../../../../plots/pete-cognition/main.conduit");
const RESUMABLE_SOURCE: &str =
    include_str!("../../../../plots/pete-cognition-resumable/main.conduit");
const CLOCK: &str = "clock/pete-cognition";
const CREATE_SOURCE: &str = "pete/cognition-create";

pub(super) fn run(args: StdCognitionArgs, dry_run: bool) -> Result<(), String> {
    if !(1..=16).contains(&args.utterance.len())
        || args
            .utterance
            .iter()
            .any(|text| text.trim().is_empty() || text.len() > 256)
    {
        return Err("supply 1 to 16 nonblank utterances of at most 256 UTF-8 bytes each".into());
    }
    if dry_run {
        println!(
            "Pete cognition will submit {} utterances to one Play.",
            args.utterance.len()
        );
        return Ok(());
    }
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
        conduit_pete::pete_cognition_situate_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::situate_pete_frame_bytes,
    )?;
    let initial = args
        .checkpoint_dir
        .as_ref()
        .map(|_| conduit_chat::initial_episode_memory());
    let initial_authoring = authoring(initial.as_deref())?;
    let (context, journal, _checkpoint_lock) = match args.checkpoint_dir {
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
    let expected_episode = completed
        .checked_add(args.utterance.len() as u64)
        .ok_or("Pete episode count exhausted")?;
    let config = StdHostConfig {
        host_id: "host/pete-cognition".into(),
        boot_id: format!("boot/pete-cognition/{}", std::process::id()).into(),
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
        Some(checkpoint) => authoring(Some(checkpoint))?,
        None => initial_authoring,
    };
    let fragment = plan(&host, &authoring, checkpoint.is_some())?;
    let seed = ExternalForeInput {
        front_port_id: "context".into(),
        track: ConnectionTrack::Payload,
        bytes: conduit_chat::encode_body_conversation_context(&context)
            .map_err(|error| format!("encode Pete context: {error:?}"))?,
    };
    let replay_out = args.replay_out;
    let mut control = RunControl::default();
    if replay_out.is_some() {
        control = control.with_debug_body(context.body_id.clone());
    }
    let queue = BodyLiveForeQueue::for_local_flow_fragment(
        &fragment,
        "frames",
        seed,
        control.clone(),
        1,
        args.utterance.len() as u16,
    )?;
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
    let input = queue.clone();
    let (delivered, receipt) = mpsc::channel();
    let producer = std::thread::spawn(move || {
        let result = (|| {
            input.wait_until_play_started()?;
            let count = args.utterance.len();
            for (index, utterance) in args.utterance.into_iter().enumerate() {
                let frame = frame(
                    provider.as_mut(),
                    completed + index as u64 + 1,
                    utterance,
                    args.retain,
                )?;
                let bytes = frame
                    .encode()
                    .map_err(|error| format!("encode Pete frame: {error:?}"))?;
                if input.submit(&bytes)?
                    != (BodyLiveForeAdmission::Accepted {
                        sequence: index as u64,
                    })
                {
                    return Err("live Fore was full after the previous episode".into());
                }
                if index + 1 < count {
                    receipt
                        .recv()
                        .map_err(|_| "Play ended before the next utterance".to_owned())?;
                }
            }
            Ok::<(), String>(())
        })();
        let closed = input.close();
        result.and(closed)
    });
    let mut output = EpisodeOutput {
        delivered,
        archive,
        journal,
        checkpoint_episode: completed,
        delivered_episode: completed,
        acknowledged_episode: completed,
    };
    let report = host.run_external_plot_live_to(
        fragment.clone(),
        &queue,
        &mut output,
        &mut std::io::stderr(),
        &mut ThreadTimer,
        &control,
    );
    let acknowledged_episode = output.acknowledged_episode;
    drop(output);
    let input_result = producer
        .join()
        .map_err(|_| "Pete input producer panicked")?;
    let report = report?;
    input_result?;
    let Some(last) = report.observations.last() else {
        return Err("Pete Play did not report a terminal disposition".into());
    };
    if !matches!(
        &last.kind,
        conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed,
        }
    ) {
        return Err(format!("Pete Play ended with {:?}", last.kind));
    }
    if acknowledged_episode != expected_episode {
        return Err(format!(
            "Pete Play acknowledged episode {acknowledged_episode}, expected {expected_episode}"
        ));
    }
    if let Some(path) = replay_out {
        use std::io::Write;
        let snapshot = super::pete_replay::snapshot(&fragment, &authoring, &context, &report)?;
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

struct EpisodeOutput {
    delivered: mpsc::Sender<()>,
    archive: Option<std::fs::File>,
    journal: Option<EpisodeCheckpointResidence>,
    checkpoint_episode: u64,
    delivered_episode: u64,
    acknowledged_episode: u64,
}

impl ExternalForeOutputAdapter for EpisodeOutput {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.front_port_id.as_str() == "checkpoints" {
            let journal = self.journal.as_ref().ok_or("unexpected Pete checkpoint")?;
            let memory = conduit_chat::decode_episode_checkpoint(&delivery.bytes)
                .map_err(|error| format!("decode Pete checkpoint: {error:?}"))?;
            journal.commit(&delivery.bytes)?;
            self.checkpoint_episode = *memory.completed();
            self.acknowledge();
            return Ok(());
        }
        let episode = conduit_chat::EpisodeRecord::decode(&delivery.bytes)
            .map_err(|error| format!("decode Pete episode: {error:?}"))?;
        if let Some(archive) = &mut self.archive {
            archive_episode(archive, &episode, &delivery.bytes)?;
        }
        match episode.response() {
            Some(response) => println!("{}", response.content()),
            None => eprintln!(
                "Episode {} ended without a model response.",
                episode.episode()
            ),
        }
        self.delivered_episode = *episode.episode();
        self.acknowledge();
        Ok(())
    }
}

impl EpisodeOutput {
    fn acknowledge(&mut self) {
        let complete = if self.journal.is_some() {
            self.checkpoint_episode.min(self.delivered_episode)
        } else {
            self.delivered_episode
        };
        if complete > self.acknowledged_episode {
            self.acknowledged_episode = complete;
            let _ = self.delivered.send(());
        }
    }
}

fn archive_episode(
    archive: &mut std::fs::File,
    episode: &conduit_chat::EpisodeRecord,
    canonical: &[u8],
) -> Result<(), String> {
    use conduit_chat::EpisodeOutcome;
    use std::io::Write;

    let evidence = episode
        .evidence()
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()
        .map_err(|error| format!("decode archived episode evidence: {error}"))?;
    let outcome = match episode.outcome() {
        EpisodeOutcome::Produced(outcome) => {
            let response = outcome.response();
            serde_json::json!({
                "kind": "produced",
                "origin": "model_derived",
                "content": response.content(),
                "implementation": response.implementation(),
                "run": response.run(),
                "truncated": response.truncated(),
            })
        }
        EpisodeOutcome::ProviderLost(outcome) => serde_json::json!({
            "kind": "provider_lost",
            "attempt": outcome.attempt(),
            "implementation": outcome.implementation(),
        }),
    };
    let recalled: Vec<_> = episode
        .recalled()
        .get()
        .iter()
        .map(|item| item.episode())
        .collect();
    let mut line = serde_json::to_vec(&serde_json::json!({
        "schema": "conduit.pete/episode-archive-row@1",
        "body_id": episode.body_id(),
        "wake_id": episode.wake_id(),
        "wake_sequence": episode.wake_sequence(),
        "episode": episode.episode(),
        "observation": {
            "identity": episode.observation().identity(),
            "content": episode.observation().content(),
        },
        "retained": episode.retained(),
        "recalled_episodes": recalled,
        "evidence": evidence,
        "outcome": outcome,
        "canonical_episode_bytes": canonical,
    }))
    .map_err(|error| format!("encode archived episode: {error}"))?;
    line.push(b'\n');
    archive
        .write_all(&line)
        .and_then(|()| archive.sync_data())
        .map_err(|error| format!("commit Pete episode archive row: {error}"))
}

fn frame(
    provider: Option<&mut StdCreateUartBase>,
    revision: u64,
    utterance: String,
    retain: bool,
) -> Result<PeteCognitionFrame, String> {
    let sign = |kind| {
        PeteCognitionSign::new(format!("pete/{kind}/{revision}"))
            .map_err(|error| format!("Pete source Sign: {error:?}"))
    };
    let utterance_at = now()?;
    let battery = match provider {
        Some(provider) => match capture_pete_cognition_battery(
            provider,
            sign("battery")?,
            CLOCK.into(),
            1_000,
            monotonic_millis,
        ) {
            Ok(Some(sample)) => sample
                .try_into()
                .map_err(|error| format!("battery input: {error:?}"))?,
            Ok(None) => PeteCognitionBatteryInput::Missing,
            Err(CreateCognitionCaptureError::Read {
                pause_failure: None,
                ..
            }) => PeteCognitionBatteryInput::unavailable(CREATE_SOURCE.into())
                .map_err(|error| format!("battery source: {error:?}"))?,
            Err(error) => return Err(format!("capture Create battery: {error:?}")),
        },
        None => PeteCognitionBatteryInput::Missing,
    };
    PeteCognitionFrame::new(
        battery,
        None,
        "pete/cognition-cli".into(),
        now()?,
        revision,
        conduit_human::ExperienceTemporalPolicy::new(5_000, 10_000)
            .map_err(|error| format!("temporal policy: {error:?}"))?,
        PeteCognitionUtterance::new(utterance, utterance_at, retain, sign("utterance")?)
            .map_err(|error| format!("human input: {error:?}"))?,
    )
    .map_err(|error| format!("Pete frame: {error:?}"))
}

fn now() -> Result<NativeTemporalInstant, String> {
    NativeTemporalInstant::new(
        CLOCK.into(),
        1,
        NativeTemporalScale::Milliseconds,
        monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?,
        0,
    )
    .map_err(|error| format!("capture instant: {error:?}"))
}

fn context(
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

fn checkpoint_context(
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

fn authoring(checkpoint: Option<&[u8]>) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile)?;
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile)?;
    conduit_chat::install_episode_loop_catalog(&mut startup, &mut profile)?;
    conduit_pete::install_pete_cognition_catalog(&mut startup, &mut profile)?;
    let (source, entry) = match checkpoint {
        Some(checkpoint) => {
            conduit_chat::install_episode_resumable_memory_catalog(
                checkpoint,
                &mut startup,
                &mut profile,
            )?;
            (RESUMABLE_SOURCE, "pete-cognition-resumable")
        }
        None => (SOURCE, "pete-cognition"),
    };
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check Pete Plot: {error:?}"))?;
    conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &profile)
        .map_err(|error| format!("expand Pete Plot: {error:?}"))
}

fn plan(
    host: &StdHost,
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    resumable: bool,
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
    let mut plan = plan_expanded_authoring_with_options(
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
