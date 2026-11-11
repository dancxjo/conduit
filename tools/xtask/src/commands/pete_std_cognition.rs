//! Operator entrance for the ordinary Pete cognition Plot.

use clap::Args;
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{BaseImplementationId, ConnectionTrack, PlanFragment, PortDirection};
use conduit_pete::{
    capture_pete_cognition_battery, CreateCognitionCaptureError, PeteCognitionBatteryInput,
    PeteCognitionFrame, PeteCognitionSign, PeteCognitionUtterance, UartProfile,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_connection_limits,
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    episode_checkpoint::EpisodeCheckpointResidence,
    hosted_local_model::OllamaDiscovery,
    std_create_uart::{monotonic_millis, StdCreateUartBase, StdCreateUartObservation},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, ExternalForeInput,
    ExternalForeOutputAdapter, HostedFrameBack, RunControl, StdHost, StdHostComposition,
    StdHostConfig, ThreadTimer,
};
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};
mod cadenced;
mod output;
mod planning;
use output::EpisodeOutput;
use planning::{authoring, checkpoint_context, context, plan};

#[derive(Args, Debug)]
pub(super) struct StdCognitionArgs {
    /// Explicit Create serial device. Omit for a session with unknown battery state.
    #[arg(long)]
    serial_path: Option<PathBuf>,
    /// One human utterance per episode, in order.
    #[arg(long, required = true)]
    utterance: Vec<String>,
    /// Retain reflected utterances for recall within this Play.
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
    /// Reflect after these 1-based utterance positions, in increasing order.
    #[arg(long)]
    reflection_after: Vec<usize>,
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
    if !args.reflection_after.is_empty() {
        cadenced::validate_positions(&args.reflection_after, args.utterance.len())?;
    }
    if dry_run {
        println!(
            "Pete cognition will submit {} utterances and {} reflection triggers to one Play.",
            args.utterance.len(),
            if args.reflection_after.is_empty() {
                args.utterance.len()
            } else {
                args.reflection_after.len()
            }
        );
        return Ok(());
    }
    if !args.reflection_after.is_empty() {
        return cadenced::run(args);
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
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::prepare_pete_experience_input_bytes,
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
    planning::install_play_states(&mut host, &authoring)?;
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
    let mut output = EpisodeOutput::new(Some(delivered), archive, journal, completed);
    let report = host.run_external_plot_live_to(
        fragment.clone(),
        &queue,
        &mut output,
        &mut std::io::stderr(),
        &mut ThreadTimer,
        &control,
    );
    let acknowledged_episode = output.acknowledged_episode();
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

fn frame(
    provider: Option<&mut StdCreateUartBase>,
    revision: u64,
    utterance: String,
    retain: bool,
) -> Result<PeteCognitionFrame, String> {
    frame_with_namespace(provider, revision, utterance, retain, "")
}
fn frame_with_namespace(
    provider: Option<&mut StdCreateUartBase>,
    revision: u64,
    utterance: String,
    retain: bool,
    namespace: &str,
) -> Result<PeteCognitionFrame, String> {
    let sign = |kind| {
        PeteCognitionSign::new(if namespace.is_empty() {
            format!("pete/{kind}/{revision}")
        } else {
            format!("pete/{kind}/{revision}/{namespace}")
        })
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
        if namespace.is_empty() {
            "pete/cognition-cli".into()
        } else {
            format!("pete/cognition-cli/{namespace}")
        },
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
