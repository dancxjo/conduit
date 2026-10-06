//! One actual checked speech Plot, realized through the installed Host and WAV effect.
use crate::cli::GlobalOpts;
use clap::Args;
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    hosted_speech_synthesis::{
        language_request_literal, read_language_coverage, read_language_request, EspeakDiscovery,
    },
    hosted_wav_artifact::WavArtifactSelection,
    StdHost, StdHostComposition, StdHostConfig, TimerAdapter,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, time::Duration};

#[derive(Args, Debug)]
pub(super) struct SpeechProofRequest {
    #[arg(long)]
    pub executable: PathBuf,
    #[arg(long)]
    pub data: PathBuf,
    #[arg(long)]
    pub engine: PathBuf,
    #[arg(long, default_value = "en-us")]
    pub voice: String,
    /// Artifact-bound native LanguageCoverage prepared for this provider.
    #[arg(long)]
    pub language_coverage: PathBuf,
    /// Exact native LanguageRequest for the authored synthesis operation.
    #[arg(long)]
    pub language_request: PathBuf,
    #[arg(long, default_value = "Hello.")]
    pub text: String,
    /// Use committed segments and incremental PCM with a 30-second work allowance.
    #[arg(long)]
    pub stream: bool,
    /// New directory for the exact Plot, Plan, WAV, and execution receipt.
    #[arg(long)]
    pub output: PathBuf,
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}

pub(super) fn prove(
    request: SpeechProofRequest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let maximum_text = if request.stream {
        1024
    } else {
        conduit_tongues::MAXIMUM_TEXT_BYTES as usize
    };
    if request.text.is_empty() || request.text.len() > maximum_text || request.text.contains('\0') {
        return Err(format!(
            "speech text must be nonempty UTF-8 of at most {maximum_text} bytes, without NUL"
        )
        .into());
    }
    if request.output.exists() {
        return Err("speech proof output must be a new directory".into());
    }
    if opts.dry_run {
        if opts.json {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.tools/real-speech-proof@1", "dry_run":true, "effects_performed":false})
            );
        } else if !opts.quiet {
            println!("Would check and execute one real speech Plot into an acknowledged WAV artifact; no playback.");
        }
        return Ok(());
    }
    let discovery = EspeakDiscovery::inspect(
        &request.executable,
        &request.data,
        &request.voice,
        &[request.engine],
    )?;
    let discovery =
        discovery.declare_language_coverage(read_language_coverage(&request.language_coverage)?)?;
    let language_request =
        language_request_literal(&read_language_request(&request.language_request)?);
    let provider_digest = discovery.provider_sha256.clone();
    // Use the ordinary Host's fresh Boot generator, with only the minimal host
    // composition and explicitly attached speech/artifact implementations.
    let fresh = StdHost::new();
    let advertisement = fresh.advertisement();
    let config = StdHostConfig {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        offer_generation: advertisement.offer_generation,
    };
    let adapter = discovery.initialize(
        config.host_id.clone(),
        config.boot_id.clone(),
        config.offer_generation,
        "grant/xtask-real-speech".into(),
        Duration::from_secs(if request.stream { 30 } else { 10 }),
    )?;
    let source = if request.stream {
        format!("plot real_speech (\n >> text: Text...| <= 1024B\n) {{\n commit: speech/commit-generated-text\n voice: speech/synthesize-stream(language-request = {language_request}, maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\", maximum-blocks = 32768, maximum-audio-millis = 30000)\n artifact: audio/play(maximum-blocks = 32768, maximum-audio-millis = 30000)\n text >> commit.generated\n commit.segments >> voice.text\n voice.audio >> convert.audio\n convert.converted >> artifact.audio\n}}.\n")
    } else {
        format!("plot real_speech {{\n voice: speech/synthesize(language-request = {language_request}, maximum-output-bytes = 131072)\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")\n artifact: audio/play\n {} >> voice.text\n voice.audio >> convert.audio\n convert.converted >> artifact.audio\n}}.\n", serde_json::to_string(&request.text)?)
    };
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
        .map_err(|error| format!("check real speech Plot: {error:?}"))?;
    let authoring = expand_canonical_plot_for_authoring(&checked, "real_speech", &profiles)
        .map_err(|error| format!("expand real speech Plot: {error:?}"))?;
    if let Some(parent) = request
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&request.output)?;
    let wav_path = request.output.join("speech.wav");
    let artifact =
        WavArtifactSelection::new(&wav_path, config.boot_id.clone(), config.offer_generation)?;
    let mut host = StdHost::new_with_composition(config, StdHostComposition::minimal().with_text());
    host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| format!("place real speech: {error:?}"))?;
    let grants = [
        if request.stream {
            host.streaming_speech_authority_grant()?
        } else {
            host.speech_synthesis_authority_grant()?
        },
        host.wav_artifact_authority_grant("grant/xtask-speech-wav")?,
    ];
    let mut boundaries = BTreeMap::new();
    if request.stream {
        boundaries.insert(
            conduit_planner::ForeBoundaryKey {
                direction: conduit_core::PortDirection::Input,
                front_port_id: "text".into(),
                track: conduit_core::ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1024,
            },
        );
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: if request.stream {
                2048
            } else {
                conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES
            },
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|error| format!("plan real speech: {error:?}"))?;
    if plan.fragments.len() != 1 {
        return Err("speech proof requires exactly one local fragment".into());
    }
    write_new(&request.output.join("plot.conduit"), source.as_bytes())?;
    write_new(
        &request.output.join("plan.json"),
        &serde_json::to_vec_pretty(&plan)?,
    )?;
    write_new(&request.output.join("words.txt"), request.text.as_bytes())?;
    struct NoOutput;
    impl conduit_std_host::ExternalForeOutputAdapter for NoOutput {
        fn deliver(&mut self, _: conduit_std_host::ExternalForeDelivery) -> Result<(), String> {
            Err("speech proof has no external output port".into())
        }
    }
    let report = if request.stream {
        host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[conduit_std_host::ExternalForeInput {
                front_port_id: "text".into(),
                track: conduit_core::ConnectionTrack::Payload,
                bytes: request.text.as_bytes().to_vec(),
            }],
            &mut NoOutput,
            &mut Vec::new(),
            &mut NoTimer,
        )?
    } else {
        host.run_fragment_to(plan.fragments[0].clone(), &mut Vec::new(), &mut NoTimer)?
    };
    if !matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ) {
        write_new(
            &request.output.join("failure.json"),
            &serde_json::to_vec_pretty(&report.observations)?,
        )?;
        return Err("real speech Play did not complete; no successful proof receipt issued".into());
    }
    let kernel = report
        .kernel
        .ok_or("real speech did not execute the installed kernel")?;
    let [wav] = kernel.wav_artifacts.as_slice() else {
        return Err("speech proof did not produce exactly one WAV artifact".into());
    };
    if !wav.completed || wav.pcm_bytes == 0 {
        return Err("speech WAV artifact was not acknowledged complete".into());
    }
    let bytes = fs::read(&wav_path)?;
    if bytes.len() != wav.pcm_bytes as usize + 44
        || bytes.get(..4) != Some(b"RIFF")
        || bytes.get(8..12) != Some(b"WAVE")
    {
        return Err("retained WAV bytes differ from runtime artifact extent".into());
    }
    let receipt = serde_json::json!({
        "schema":"conduit.tools/real-speech-proof@1", "proof_class":"host-execution-audio-artifact", "dry_run":false,
        "streaming":request.stream, "effects_performed":true, "playback_performed":false, "human_listening_proved":false,
        "source_document_id":checked.source_document_id, "checked_plot_id":authoring.expanded.checked_plot_id,
        "provider_sha256":provider_digest, "text_sha256":format!("{:x}",Sha256::digest(request.text.as_bytes())),
        "host_id":hosts[0].host_id, "boot_id":hosts[0].boot_id, "plan_id":plan.plan_id,
        "active_play":kernel.active_play, "active_play_id":kernel.active_play_id,
        "wav":{"path":"speech.wav", "sha256":format!("{:x}",Sha256::digest(&bytes)), "bytes":bytes.len(),
            "pcm_bytes":wav.pcm_bytes, "frames":wav.frames, "blocks":wav.blocks, "completed":wav.completed},
        "observations":report.observations,
    });
    write_new(
        &request.output.join("receipt.json"),
        &serde_json::to_vec_pretty(&receipt)?,
    )?;
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!(
            "Real speech Plan/Play completed; WAV retained at {}. No playback or listening claim.",
            wav_path.display()
        );
    }
    Ok(())
}
fn write_new(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)
}
