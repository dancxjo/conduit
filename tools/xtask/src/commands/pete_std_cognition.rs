//! Operator entrance for the ordinary Pete cognition Plot.

use clap::Args;
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{BaseImplementationId, ConnectionTrack, PlanFragment, PortDirection};
use conduit_pete::{
    capture_pete_cognition_battery, CreateCognitionCaptureError, PeteCognitionBatteryInput,
    PeteCognitionFrame, PeteCognitionSign, PeteCognitionUtterance, UartProfile,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    hosted_local_model::OllamaDiscovery,
    std_create_uart::{monotonic_millis, StdCreateUartBase, StdCreateUartObservation},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, ExternalForeInput,
    ExternalForeOutputAdapter, HostedFrameBack, RunControl, StdHost, StdHostComposition,
    StdHostConfig, ThreadTimer,
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
}

const SOURCE: &str = include_str!("../../../../plots/pete-cognition/main.conduit");
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
    let mut host = StdHost::new_with_local_model_and_frame_back(
        StdHostConfig {
            host_id: "host/pete-cognition".into(),
            boot_id: format!("boot/pete-cognition/{}", std::process::id()).into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(model),
        back,
    )?;
    let fragment = plan(&host)?;
    let seed = context(&fragment)?;
    let control = RunControl::default();
    let queue = BodyLiveForeQueue::for_local_flow_fragment(
        &fragment,
        "frames",
        seed,
        control.clone(),
        1,
        args.utterance.len() as u16,
    )?;
    let input = queue.clone();
    let (delivered, receipt) = mpsc::channel();
    let producer = std::thread::spawn(move || {
        let result = (|| {
            input.wait_until_play_started()?;
            let count = args.utterance.len();
            for (index, utterance) in args.utterance.into_iter().enumerate() {
                let frame = frame(provider.as_mut(), index as u64 + 1, utterance, args.retain)?;
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
    let mut output = EpisodeOutput(delivered);
    let report = host.run_external_plot_live_to(
        fragment,
        &queue,
        &mut output,
        &mut std::io::stderr(),
        &mut ThreadTimer,
        &control,
    );
    drop(output);
    producer
        .join()
        .map_err(|_| "Pete input producer panicked")??;
    let report = report?;
    let Some(last) = report.observations.last() else {
        return Err("Pete Play did not report a terminal disposition".into());
    };
    match &last.kind {
        conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed,
        } => Ok(()),
        other => Err(format!("Pete Play ended with {other:?}")),
    }
}

struct EpisodeOutput(mpsc::Sender<()>);

impl ExternalForeOutputAdapter for EpisodeOutput {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        let episode = conduit_chat::EpisodeRecord::decode(&delivery.bytes)
            .map_err(|error| format!("decode Pete episode: {error:?}"))?;
        match episode.response() {
            Some(response) => println!("{}", response.content()),
            None => eprintln!(
                "Episode {} ended without a model response.",
                episode.episode()
            ),
        }
        let _ = self.0.send(());
        Ok(())
    }
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

fn context(fragment: &PlanFragment) -> Result<ExternalForeInput, String> {
    let body = Body::born(
        fragment.source_document_id.clone(),
        fragment.checked_plot_id.clone(),
        1,
        format!("sign/pete/born/{}", std::process::id()).into(),
    )
    .map_err(|error| format!("birth Pete: {error:?}"))?;
    let (_, wake) = body
        .wake(2, format!("sign/pete/wake/{}", std::process::id()).into())
        .map_err(|error| format!("wake Pete: {error:?}"))?;
    let bytes = conduit_chat::encode_body_conversation_context(&BodyConversationContext {
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
        active_plots: vec!["pete-cognition".into()],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    })
    .map_err(|error| format!("encode Pete context: {error:?}"))?;
    Ok(ExternalForeInput {
        front_port_id: "context".into(),
        track: ConnectionTrack::Payload,
        bytes,
    })
}

fn plan(host: &StdHost) -> Result<PlanFragment, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile)?;
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile)?;
    conduit_chat::install_episode_loop_catalog(&mut startup, &mut profile)?;
    conduit_pete::install_pete_cognition_catalog(&mut startup, &mut profile)?;
    let syntax = conduit_plot::parse_syntax_document(SOURCE);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check Pete Plot: {error:?}"))?;
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "pete-cognition", &profile)
            .map_err(|error| format!("expand Pete Plot: {error:?}"))?;
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
        &authoring,
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
