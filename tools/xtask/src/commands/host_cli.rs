//! Typed Host construction and proof entrances.
use super::{host_esp32_inspection, host_release};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct HostArgs {
    #[command(subcommand)]
    pub(super) command: Option<HostCommand>,
}

#[derive(Subcommand, Debug)]
pub(super) enum HostCommand {
    /// Launch the ordinary std Host (the default Host target).
    Std,
    /// Build and launch one independent browser page/WASM Host.
    Browser,
    /// Build, flash, or physically verify an exact Raspberry Pi Host IMAGE.
    Rpi(RpiHostArgs),
    /// Create or revise one canonical Host construction document interactively.
    Configure {
        /// Existing configuration to edit, or destination offered when creating.
        path: Option<PathBuf>,
    },
    /// Check or display one canonical Host construction document.
    Config {
        #[command(subcommand)]
        command: HostConfigCommand,
    },
    /// Emit the checked target and Base chooser catalog as portable JSON.
    Catalog,
    /// Resolve one PROFILE and emit its exact IMAGE and build manifest.
    Build {
        profile: PathBuf,
        #[arg(long, default_value = "target/host-build")]
        output: PathBuf,
        /// Exact source identity; defaults to the current Git commit.
        #[arg(long)]
        source_identity: Option<String>,
    },
    /// Compile and seal the reviewed generic existing-computer release bundles.
    Release {
        #[arg(long, default_value = "target/creche-host-releases")]
        output: PathBuf,
        /// Native platform to compile on this runner.
        #[arg(long, value_enum, default_value_t = host_release::ReleasePlatform::Linux)]
        platform: host_release::ReleasePlatform,
        /// Exact source identity; defaults to the current Git commit.
        #[arg(long)]
        source_identity: Option<String>,
    },
    /// Seal reviewed release manifests into the installed target catalog.
    ReleaseCatalog {
        /// Directory containing reviewed release manifests and their artifacts.
        #[arg(long)]
        root: PathBuf,
        /// Monotonically increasing release-channel generation.
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        generation: u64,
    },
    /// Package one exact reviewed BrowserBundle for the zero-dependency ESM SDK.
    BrowserSdkPackage {
        /// Directory containing the sealed BrowserBundle and IMAGE.
        #[arg(long)]
        bundle: PathBuf,
        /// New output directory for the npm-style package.
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify one final target IMAGE and its exact BUILD closure.
    Verify {
        output: PathBuf,
        /// Boot the verified IMAGE through the deterministic x86_64 QEMU appliance.
        #[arg(long)]
        boot: bool,
    },
    /// Prove one body across native, browser, and headless PROFILE-built Hosts.
    Capstone {
        #[arg(long, default_value = "target/host-make-capstone")]
        output: PathBuf,
        #[arg(long, default_value = "workspace-head")]
        source_identity: String,
    },
    /// Inspect one attached ESP32 without writing its flash.
    InspectEsp32 {
        /// Stable serial device path, preferably beneath /dev/serial/by-id.
        #[arg(long)]
        port: PathBuf,
        /// Exact SoC class expected from the attached board.
        #[arg(long, value_enum)]
        expected_soc: host_esp32_inspection::Esp32SocClass,
        /// Literal text observed on the development-board PCB.
        #[arg(long)]
        board_marking: String,
        /// Literal text observed on the module's RF shield.
        #[arg(long)]
        module_marking: String,
        /// Literal board revision, or `unmarked` when inspection finds none.
        #[arg(long)]
        board_revision: String,
        #[arg(long, default_value = "target/esp32-inspection/inspection.json")]
        output: PathBuf,
    },
    /// Inspect one already-local Ollama model without loading or downloading it.
    InspectLocalModel {
        /// Exact local model name or its local `:latest` alias.
        #[arg(long)]
        model: String,
        /// Exact loopback Ollama origin.
        #[arg(long, default_value = conduit_std_host::hosted_local_model::DEFAULT_OLLAMA_ENDPOINT)]
        ollama_endpoint: String,
    },
    /// Initialize and warm one already-local Ollama model under finite Host limits.
    ProveLocalModel {
        /// Exact local model name or its local `:latest` alias.
        #[arg(long)]
        model: String,
        /// Exact loopback Ollama origin.
        #[arg(long, default_value = conduit_std_host::hosted_local_model::DEFAULT_OLLAMA_ENDPOINT)]
        ollama_endpoint: String,
        /// Finite admitted RAM/VRAM ceiling expressed in MiB.
        #[arg(long)]
        admitted_memory_mib: u32,
        /// Run the fixed-policy Orifina comparison over one exact Workspace tutorial state.
        #[arg(long)]
        orifina_presenter: bool,
        /// Retain every tutorial-state manifestation for an explicit documentary run.
        #[arg(long, requires = "orifina_presenter")]
        journey_documentary: bool,
        /// Explicit local speech provider for retained runtime-produced WAVs.
        #[command(flatten)]
        speech: super::host_speech::SpeechOptions,
    },
    /// Produce a bounded WAV through ordinary real speech Plan/Play, without playback.
    ProveSpeech(super::host_speech_proof::SpeechProofRequest),
    /// Speak through Tongues as Host, perform one Birth, then speak as the Body.
    ProveSpokenBirth {
        /// Explicitly consume the one allowed Birth action.
        #[arg(long)]
        confirm_birth: bool,
        #[arg(long, default_value = "No Body exists yet. Confirm to begin.")]
        bootstrap_text: String,
        #[arg(long, default_value = "I am now speaking as the born Body.")]
        body_text: String,
    },
    /// Carry a finite recorded PCM clip through ordinary Whisper Plan/Play.
    ProveWhisper {
        /// Exact local whisper.cpp-compatible executable.
        #[arg(long)]
        executable: PathBuf,
        /// Exact already-local Whisper model file.
        #[arg(long)]
        model: PathBuf,
        /// Raw mono signed-16-le 16 kHz PCM recording, at most six seconds.
        #[arg(long)]
        pcm_s16le_16000_mono: PathBuf,
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=32))]
        threads: u8,
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120))]
        timeout_seconds: u64,
    },
    /// Explicitly capture one bounded microphone clip and recognize it through Whisper.
    ProveMicrophoneWhisper {
        #[arg(long)]
        arecord_executable: PathBuf,
        #[arg(long)]
        card_id: String,
        #[arg(long)]
        device: u16,
        #[arg(long, default_value_t = 3_000)]
        capture_milliseconds: u32,
        #[arg(long, default_value_t = 10)]
        capture_timeout_seconds: u64,
        #[arg(long)]
        whisper_executable: PathBuf,
        #[arg(long)]
        whisper_model: PathBuf,
        #[arg(long, default_value_t = 2)]
        whisper_threads: u8,
        #[arg(long, default_value_t = 30)]
        whisper_timeout_seconds: u64,
        #[arg(long)]
        authorize_capture: bool,
    },
}

#[derive(Args, Debug)]
pub(super) struct RpiHostArgs {
    /// Exact currently supported Raspberry Pi board profile.
    #[arg(long, value_enum, default_value_t = crate::commands::conduitos::Armv6RpiBoard::default())]
    pub(super) board: crate::commands::conduitos::Armv6RpiBoard,

    #[command(subcommand)]
    pub(super) action: Option<RpiHostAction>,
}

#[derive(Subcommand, Debug)]
pub(super) enum RpiHostAction {
    /// Generate and independently verify the exact SD-card IMAGE (default).
    Image,
    /// Erase, write, and byte-verify one explicitly confirmed removable device.
    Flash {
        /// Exact whole removable block device to erase and write.
        #[arg(long)]
        device: PathBuf,
        /// Repeat the exact device path to acknowledge destructive erasure.
        #[arg(long)]
        confirm_device: PathBuf,
    },
    /// Capture and validate one exact physical Raspberry Pi UART boot.
    PhysicalProof {
        /// Exact UART character device connected through a 3.3 V TTL adapter.
        #[arg(long)]
        serial_device: PathBuf,
        /// Finite capture deadline in seconds.
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120))]
        timeout_seconds: u64,
    },
}

#[derive(Subcommand, Debug)]
pub(super) enum HostConfigCommand {
    /// Validate canonical source without saving or making an IMAGE.
    Check { path: PathBuf },
    /// Print the resolved target, Bases, variants, limits, and identity.
    Show { path: PathBuf },
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        host: HostArgs,
    }

    #[test]
    fn documentary_speech_requires_an_explicit_complete_provider() {
        let base = [
            "host",
            "prove-local-model",
            "--model",
            "local",
            "--admitted-memory-mib",
            "1024",
        ];
        assert!(Cli::try_parse_from(base).is_ok());
        let mut args = base.to_vec();
        args.extend(["--speech-executable", "/engine"]);
        assert!(Cli::try_parse_from(&args).is_err());
        args.extend([
            "--speech-data",
            "/data",
            "--speech-engine",
            "/libengine",
            "--orifina-presenter",
            "--journey-documentary",
        ]);
        assert!(Cli::try_parse_from(&args).is_ok());
    }
}
