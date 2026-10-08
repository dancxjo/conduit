pub use crate::commands::audio::cli::{AudioPlaybackArgs, StartupCueArgs};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::commands::avr::AvrArgs;
use crate::commands::body::BodyArgs;
use crate::commands::body_coordination::BodyCoordinationArgs;
use crate::commands::check::CheckArgs;
use crate::commands::ci::CiArgs;
use crate::commands::conduitos::ConduitosArgs;
use crate::commands::esp32_firmware::Esp32FirmwareArgs;
use crate::commands::evidence::EvidenceCommand;
use crate::commands::handbook::{HandbookArgs, PagesRootArgs};
use crate::commands::host::HostArgs;
use crate::commands::pete_std_observe::PeteArgs;
use crate::commands::pico::PicoArgs;

/// Repository orchestration task runner for Conduit.
#[derive(Parser, Debug)]
#[command(name = "xtask", about = "Conduit repository orchestration")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOpts,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone, Default)]
pub struct GlobalOpts {
    /// Print planned probes or commands without executing them.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress non-error human output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Emit one structured JSON report to stdout.
    #[arg(long, global = true)]
    pub json: bool,

    /// Forward --locked to Cargo commands.
    #[arg(long, global = true)]
    pub locked: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Execute repository validation check suites.
    Check(CheckArgs),
    /// Run the fast, local end-to-end developer truth loop.
    Integrate,
    /// Plan repository CI obligations from an exact change.
    Ci(CiArgs),
    /// Construct repository artifacts for an exact target.
    Make(MakeArgs),
    /// Execute proofs and manage their bounded evidence.
    Prove(Box<ProveArgs>),
    /// Inspect repository and platform prerequisites.
    Doctor(DoctorArgs),
    /// Install explicit prerequisites for a repository workflow.
    Setup(SetupArgs),
}

#[derive(Args, Debug)]
pub struct MakeArgs {
    #[command(subcommand)]
    pub target: MakeTarget,
}

#[derive(Subcommand, Debug)]
pub enum MakeTarget {
    /// Build or guardedly flash the Pete Pro Micro Brainstem.
    Avr(AvrArgs),
    /// Build and launch one independent browser page/WASM Host.
    Browser,
    /// Create, inspect, build, or deploy one whole Body description.
    Body(BodyArgs),
    /// Check the standalone ESP32 make package without touching hardware.
    Esp32Firmware(Esp32FirmwareArgs),
    /// Target one host lifecycle or manage exact host configuration and make.
    Host(HostArgs),
    /// Build, flash, or verify the Pico W local Signal proof.
    Pico(PicoArgs),
    /// Run the complete Pico W local workflow.
    PicoLocal(PicoArgs),
    /// Build and prove the freestanding ConduitOS reference Host.
    Conduitos(ConduitosArgs),
    /// Render the versioned wiki source as the static website handbook.
    Handbook(HandbookArgs),
    /// Stage the complete public Pages root, including the handbook.
    PagesRoot(PagesRootArgs),
    /// Generate the shared bounded GNU Unifont subset.
    UnifontSubset(UnifontSubsetArgs),
    /// Generate the bounded native masks for canonical palette icons.
    PaletteIcons(PaletteIconsArgs),
    /// Render the bounded startup cue to a new WAV file without opening audio.
    StartupCue(StartupCueArgs),
}

#[derive(Args, Debug)]
pub struct UnifontSubsetArgs {
    /// Checksum-verified upstream GNU Unifont .hex.gz file.
    pub input: std::path::PathBuf,

    /// Destination for the filtered GNU Unifont .hex file.
    pub output: std::path::PathBuf,
}

#[derive(Args, Debug)]
pub struct PaletteIconsArgs {
    /// Directory containing the pinned, repository-owned Lucide SVG subset.
    pub input: std::path::PathBuf,

    /// Destination Rust module for the deterministic 16x16 masks.
    pub output: std::path::PathBuf,
}

#[derive(Args, Debug)]
pub struct ProveArgs {
    /// Which proof suite to execute.
    pub proof: Option<ProveTarget>,

    /// Run a structured proof or produce, verify, or publish bounded proof evidence.
    #[command(subcommand)]
    pub command: Option<ProveCommand>,

    /// List the versioned proof command contract instead of executing a proof.
    #[arg(
        long,
        conflicts_with_all = ["proof", "verify_record", "run_obligation"]
    )]
    pub list: bool,

    /// Verify one JSON proof record against its exact registered command contract.
    #[arg(
        long = "verify",
        conflicts_with_all = ["proof", "list", "run_obligation"]
    )]
    pub verify_record: Option<std::path::PathBuf>,

    /// Run the one pinned finite proof-catalog validation obligation.
    #[arg(
        long,
        conflicts_with_all = ["proof", "list", "verify_record"]
    )]
    pub run_obligation: bool,

    /// Stop after emitting the reviewed checkpoint and residual obligation.
    #[arg(long, requires = "run_obligation")]
    pub interrupt_after_checkpoint: bool,

    /// Resume from one bounded checkpoint JSON file.
    #[arg(long, requires = "run_obligation")]
    pub resume: Option<std::path::PathBuf>,

    /// Write the checkpoint or terminal obligation record as bounded JSON.
    #[arg(long, requires = "run_obligation")]
    pub obligation_record: Option<std::path::PathBuf>,

    /// Override the bounded evidence root for proofs that declare evidence outputs.
    #[arg(long)]
    pub evidence_root: Option<std::path::PathBuf>,

    /// Import one bounded live Workspace worker-pool receipt. Valid only for
    /// `prove local-model-pool`; deterministic proof still runs separately.
    #[arg(long)]
    pub live_receipt: Option<std::path::PathBuf>,

    /// Explicit USB CDC link port (CDC 0).
    #[arg(long)]
    pub link_port: Option<String>,

    /// Explicit USB CDC sign port (CDC 1).
    #[arg(long)]
    pub sign_port: Option<String>,

    /// Run interactive button console control mode.
    #[arg(long)]
    pub interactive: bool,

    /// Corrupt the first planned Signal after kernel emission and require an
    /// honest two-sided sink-failure terminal instead of success.
    #[arg(long)]
    pub induce_sink_failure: bool,

    /// Connect the physical Bluetooth Line and require explicit transport-loss
    /// evidence instead of successful message delivery.
    #[arg(long)]
    pub induce_transport_loss: bool,

    /// Fail the Patchbay proof after its first canonical capture so the
    /// restarted-worker diagnostic evidence path can be verified.
    #[arg(long)]
    pub induce_capture_restart_failure: bool,

    /// Fail browser-host proof before canonical capture begins and retain a
    /// verifier-ready diagnostic manifest with no invented capture outputs.
    #[arg(long)]
    pub induce_pre_capture_failure: bool,

    /// Environment variable containing the Wi-Fi SSID. The variable value is
    /// never printed.
    #[arg(long)]
    pub ssid_env: Option<String>,

    /// Environment variable containing the Wi-Fi credential. The variable
    /// value is never printed.
    #[arg(long)]
    pub credential_env: Option<String>,

    /// Environment variable containing bounded calendar proof configuration JSON.
    #[arg(long)]
    pub calendar_config_env: Option<String>,

    /// Environment variable containing bounded GitHub messaging proof configuration JSON.
    #[arg(long)]
    pub messaging_config_env: Option<String>,

    /// Explicit Ollama HTTP endpoint used by the live planning-advice proof.
    #[arg(long)]
    pub ollama_url: Option<String>,

    /// Exact installed Ollama model used by the live planning-advice proof.
    #[arg(long)]
    pub ollama_model: Option<String>,

    /// Exact Wi-Fi client interface used for the physical Pico appliance proof.
    #[arg(long)]
    pub client_interface: Option<String>,

    /// Exact pre-flash CDC 0 port for the second Pico appliance HIL client.
    #[arg(long)]
    pub client_link_port: Option<String>,

    /// Exact post-flash CDC 1 port for the second Pico appliance HIL client.
    #[arg(long)]
    pub client_sign_port: Option<String>,

    /// Side of the exact two-Host Bluetooth proof.
    #[arg(long, value_enum)]
    pub bluetooth_role: Option<BluetoothProofRole>,

    /// Exact local BlueZ controller name, such as hci0.
    #[arg(long)]
    pub bluetooth_adapter: Option<String>,

    /// Exact paired peer Bluetooth address for this proof run.
    #[arg(long)]
    pub bluetooth_peer_address: Option<String>,

    /// Exact peer Host identity advertised by the constrained boot.
    #[arg(long)]
    pub bluetooth_peer_host_id: Option<String>,

    /// Exact peer Boot identity advertised by the constrained boot.
    #[arg(long)]
    pub bluetooth_peer_boot_id: Option<String>,

    /// Exact classic ESP32 address for the distributed Lenia proof.
    #[arg(long)]
    pub lenia_wroom_address: Option<String>,
    /// Exact classic ESP32 Boot identity for the distributed Lenia proof.
    #[arg(long)]
    pub lenia_wroom_boot: Option<String>,
    /// Exact ESP32-C3 address for the distributed Lenia proof.
    #[arg(long)]
    pub lenia_c3_address: Option<String>,
    /// Exact ESP32-C3 Boot identity for the distributed Lenia proof.
    #[arg(long)]
    pub lenia_c3_boot: Option<String>,
    /// Exact Pico W address for the distributed Lenia proof.
    #[arg(long)]
    pub lenia_pico_address: Option<String>,
    /// Exact Pico W Boot identity for the distributed Lenia proof.
    #[arg(long)]
    pub lenia_pico_boot: Option<String>,
    /// Withhold the Pico region and require honest non-completion.
    #[arg(long)]
    pub withhold_lenia_pico: bool,
}

#[derive(Subcommand, Debug)]
pub enum ProveCommand {
    /// Run the bounded audible specimen through one exact selected output.
    AudioPlayback(AudioPlaybackArgs),
    /// Prove bounded Pete forebrain-motherbrain coordination.
    BodyCoordination(BodyCoordinationArgs),
    /// Exercise explicit Pete hardware proof entrances.
    Pete(PeteArgs),
    /// Exercise one reviewed Plot or journey through its exact repository proof path.
    Journey(DemoArgs),
    /// Capture a live, partial Todo owner/terminal encounter without publishing a journey.
    TodoJourney(TodoJourneyArgs),
    /// Produce, verify, or publish bounded proof evidence.
    #[command(flatten)]
    Evidence(EvidenceCommand),
}

#[derive(Args, Debug)]
pub struct TodoJourneyArgs {
    /// Installed owner state whose current Face will be observed.
    #[arg(long)]
    pub state_dir: std::path::PathBuf,
    /// Executable from the reviewed Host release that owns this installation.
    #[arg(long)]
    pub conduit_bin: std::path::PathBuf,
    /// New directory in which to retain the exact command outputs.
    #[arg(long)]
    pub output: std::path::PathBuf,
    /// UTF-8 terminal commands (defaults to one read followed by quit).
    #[arg(long)]
    pub terminal_script: Option<std::path::PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothProofRole {
    Source,
    Sink,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProveTarget {
    BluetoothLine,
    BluetoothPico,
    BodyMembership,
    BodyMembershipHil,
    StdBrowserS4,
    StdBrowserToggle,
    BrowserHost,
    CalendarGoogle,
    DegradedProfiles,
    Diversity,
    ResourceFrame,
    DistributedLenia,
    DormantReadmission,
    RecursiveRecovery,
    EmergencyControl,
    LlmEmbodiment,
    LlmCrossHost,
    LocalModelPool,
    LlmPlanningAdvice,
    MessagingGithub,
    PatchbayBodyWorkbench,
    PatchbayFrontDoor,
    StdPicoUsb,
    PicoWifiBootstrap,
    PicoAppliance,
    PicoApplianceHil,
    PicoWebsocketRoute,
    R1NewPlanRecovery,
    R1NewPlanRecoveryHil,
    R1PlanCContinuationHil,
    R1Hil,
}

mod demo;
pub use demo::{
    DemoArgs, DemoCommand, LightSwitchDemoArgs, NativeSpeechArgs, PatchbayDemoArgs, PatchbayHost,
};

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// What to inspect (default: all).
    #[arg(default_value = "all")]
    pub target: DoctorTarget,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorTarget {
    All,
    Audio,
    Browser,
    Midi,
    Pico,
    LinuxRelease,
}

impl DoctorTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Audio => "audio",
            Self::Browser => "browser",
            Self::Midi => "midi",
            Self::Pico => "pico",
            Self::LinuxRelease => "linux-release",
        }
    }
}

#[derive(Args, Debug)]
pub struct SetupArgs {
    /// Repository workflow to prepare.
    #[arg(default_value = "linux-release")]
    pub target: SetupTarget,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupTarget {
    LinuxRelease,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_and_pico_commands_parse() {
        let matrix = Cli::try_parse_from(["xtask", "check", "catalog", "matrix"])
            .expect("catalog matrix command parses");
        assert!(matches!(matrix.command, Command::Check(_)));

        let gap = Cli::try_parse_from(["xtask", "check", "catalog", "gap", "--host", "pico"])
            .expect("catalog gap command parses");
        assert!(matches!(gap.command, Command::Check(_)));

        let plots = Cli::try_parse_from(["xtask", "check", "plots", "check"])
            .expect("reviewed plots check parses beneath check");
        assert!(matches!(plots.command, Command::Check(_)));
        assert!(Cli::try_parse_from(["xtask", "catalog", "matrix"]).is_err());
        assert!(Cli::try_parse_from(["xtask", "plots", "check"]).is_err());

        let doctor = Cli::try_parse_from(["xtask", "--dry-run", "doctor", "pico"])
            .expect("doctor command parses");
        assert!(doctor.global.dry_run);
        assert!(matches!(doctor.command, Command::Doctor(_)));

        let release_doctor = Cli::try_parse_from(["xtask", "doctor", "linux-release"])
            .expect("Linux release doctor command parses");
        assert!(matches!(release_doctor.command, Command::Doctor(_)));

        let setup = Cli::try_parse_from(["xtask", "setup", "linux-release"])
            .expect("Linux release setup command parses");
        assert!(matches!(setup.command, Command::Setup(_)));

        let pico =
            Cli::try_parse_from(["xtask", "make", "pico", "build"]).expect("pico command parses");
        assert!(matches!(
            pico.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Pico(_)
            })
        ));

        let host = Cli::try_parse_from([
            "xtask",
            "make",
            "host",
            "build",
            "profile.json",
            "--output",
            "target/host-image",
            "--source-identity",
            "git:abc",
        ])
        .expect("host BUILD command parses");
        assert!(matches!(
            host.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Host(_)
            })
        ));

        let body_new = Cli::try_parse_from([
            "xtask",
            "make",
            "body",
            "new",
            "pete",
            "--template",
            "robot",
            "--host",
            "eyes=browser-page",
        ])
        .expect("Body scaffold command parses");
        assert!(matches!(
            body_new.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Body(_)
            })
        ));
        assert!(Cli::try_parse_from([
            "xtask",
            "make",
            "body",
            "new",
            "pete",
            "--template",
            "unknown"
        ])
        .is_err());
        let guided_body = Cli::try_parse_from(["xtask", "make", "body", "new"])
            .expect("interactive Body scaffold may prompt for its name");
        assert!(matches!(
            guided_body.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Body(_)
            })
        ));
        let scripted_body =
            Cli::try_parse_from(["xtask", "make", "body", "new", "pete", "--no-interactive"])
                .expect("scripted Body scaffold parses");
        assert!(matches!(
            scripted_body.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Body(_)
            })
        ));

        for command in [
            vec!["xtask", "make", "host"],
            vec!["xtask", "make", "host", "std"],
            vec!["xtask", "make", "host", "browser"],
            vec!["xtask", "make", "host", "rpi"],
            vec![
                "xtask",
                "make",
                "host",
                "rpi",
                "--board",
                "rpi-zero-v1",
                "image",
            ],
            vec![
                "xtask",
                "make",
                "host",
                "rpi",
                "flash",
                "--device",
                "/dev/sda",
                "--confirm-device",
                "/dev/sda",
            ],
            vec![
                "xtask",
                "make",
                "host",
                "rpi",
                "physical-proof",
                "--serial-device",
                "/dev/ttyUSB0",
            ],
        ] {
            let parsed = Cli::try_parse_from(command.clone())
                .unwrap_or_else(|error| panic!("host command {command:?} must parse: {error}"));
            assert!(matches!(
                parsed.command,
                Command::Make(MakeArgs {
                    target: MakeTarget::Host(_)
                })
            ));
        }
        assert!(Cli::try_parse_from([
            "xtask", "make", "host", "rpi", "flash", "--device", "/dev/sda",
        ])
        .is_err());
        assert!(
            Cli::try_parse_from(["xtask", "make", "host", "rpi", "--board", "rpi-5",]).is_err()
        );

        let pico_body = Cli::try_parse_from([
            "xtask",
            "make",
            "pico",
            "prove-body-admission",
            "--link-port",
            "/dev/serial/by-id/pico",
        ])
        .expect("physical Pico Body admission proof parses");
        assert!(matches!(
            pico_body.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Pico(PicoArgs {
                    subcommand: Some(crate::commands::pico::PicoSubcommand::ProveBodyAdmission),
                    ..
                })
            })
        ));

        let pico_build_remote =
            Cli::try_parse_from(["xtask", "make", "pico", "build", "--usb-remote"])
                .expect("pico build --usb-remote parses");
        if let Command::Make(MakeArgs {
            target: MakeTarget::Pico(args),
        }) = pico_build_remote.command
        {
            assert!(args.usb_remote);
        } else {
            panic!("expected Command::Pico");
        }

        let pico_flash_remote =
            Cli::try_parse_from(["xtask", "make", "pico", "flash", "--usb-remote"])
                .expect("pico flash --usb-remote parses");
        if let Command::Make(MakeArgs {
            target: MakeTarget::Pico(args),
        }) = pico_flash_remote.command
        {
            assert!(args.usb_remote);
        } else {
            panic!("expected Command::Pico");
        }

        let pico_build_control =
            Cli::try_parse_from(["xtask", "make", "pico", "build", "--r1-control"])
                .expect("pico build --r1-control parses");
        if let Command::Make(MakeArgs {
            target: MakeTarget::Pico(args),
        }) = pico_build_control.command
        {
            assert!(args.r1_control);
        } else {
            panic!("expected Command::Pico");
        }

        let toggle = Cli::try_parse_from(["xtask", "prove", "journey", "toggle"])
            .expect("toggle journey command parses");
        assert!(matches!(
            toggle.command,
            Command::Prove(args)
                if matches!(
                    args.command,
                    Some(ProveCommand::Journey(DemoArgs {
                        command: DemoCommand::Toggle
                    }))
                )
        ));

        for command in ["std", "triple", "patchbay", "body-membership"] {
            Cli::try_parse_from(["xtask", "prove", "journey", command])
                .unwrap_or_else(|error| panic!("journey {command} must parse: {error}"));
        }
        let browser =
            Cli::try_parse_from(["xtask", "make", "browser"]).expect("browser Host parses");
        assert!(matches!(
            browser.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Browser
            })
        ));
        assert!(Cli::try_parse_from(["xtask", "prove", "journey", "browser"]).is_err());

        let site = Cli::try_parse_from(["xtask", "prove", "journey", "site"])
            .expect("site journey command parses");
        assert!(matches!(
            site.command,
            Command::Prove(args)
                if matches!(
                    args.command,
                    Some(ProveCommand::Journey(DemoArgs {
                        command: DemoCommand::Site
                    }))
                )
        ));
        assert!(Cli::try_parse_from(["xtask", "demo", "site"]).is_err());

        let subset = Cli::try_parse_from([
            "xtask",
            "make",
            "unifont-subset",
            "unifont.hex.gz",
            "subset.hex",
        ])
        .expect("unifont-subset command parses");
        assert!(matches!(
            subset.command,
            Command::Make(MakeArgs {
                target: MakeTarget::UnifontSubset(_)
            })
        ));

        let icons = Cli::try_parse_from([
            "xtask",
            "make",
            "palette-icons",
            "mechanisms/implementations/bounded-lucide/svg",
            "icons.rs",
        ])
        .expect("palette-icons command parses");
        assert!(matches!(
            icons.command,
            Command::Make(MakeArgs {
                target: MakeTarget::PaletteIcons(_)
            })
        ));

        let check =
            Cli::try_parse_from(["xtask", "check", "workspace"]).expect("check command parses");
        assert!(matches!(check.command, Command::Check(_)));

        let ci = Cli::try_parse_from(["xtask", "ci", "standalone-locks"])
            .expect("standalone lock check command parses");
        assert!(matches!(ci.command, Command::Ci(_)));

        let prove = Cli::try_parse_from(["xtask", "prove", "std-browser-s4"])
            .expect("prove command parses");
        assert!(matches!(prove.command, Command::Prove(_)));

        let body_coordination = Cli::try_parse_from([
            "xtask",
            "prove",
            "body-coordination",
            "conformance",
            "--forebrain-boot",
            "forebrain-boot",
            "--motherbrain-boot",
            "motherbrain-boot",
            "--admit-parts",
        ])
        .expect("body coordination proof parses beneath prove");
        assert!(matches!(
            body_coordination.command,
            Command::Prove(args)
                if matches!(args.command, Some(ProveCommand::BodyCoordination(_)))
        ));
        assert!(Cli::try_parse_from([
            "xtask",
            "body-coordination",
            "conformance",
            "--forebrain-boot",
            "forebrain-boot",
            "--motherbrain-boot",
            "motherbrain-boot",
            "--admit-parts",
        ])
        .is_err());

        let calendar = Cli::try_parse_from([
            "xtask",
            "prove",
            "calendar-google",
            "--credential-env",
            "GOOGLE_TOKEN",
            "--calendar-config-env",
            "CALENDAR_CONFIG",
        ])
        .expect("calendar live proof command parses");
        assert!(matches!(
            calendar.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::CalendarGoogle)
        ));

        let messaging = Cli::try_parse_from([
            "xtask",
            "prove",
            "messaging-github",
            "--credential-env",
            "GITHUB_TOKEN",
            "--messaging-config-env",
            "MESSAGING_CONFIG",
        ])
        .expect("GitHub messaging live proof command parses");
        assert!(matches!(
            messaging.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::MessagingGithub)
        ));

        let planning_advice = Cli::try_parse_from([
            "xtask",
            "prove",
            "llm-planning-advice",
            "--ollama-url",
            "http://forebrain.local:11434",
            "--ollama-model",
            "gpt-oss:20b",
        ])
        .expect("live Ollama planning-advice command parses");
        assert!(matches!(
            planning_advice.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::LlmPlanningAdvice)
        ));

        let embodiment = Cli::try_parse_from([
            "xtask",
            "prove",
            "llm-embodiment",
            "--ollama-url",
            "http://forebrain.local:11434",
            "--ollama-model",
            "gpt-oss:20b",
        ])
        .expect("live Ollama embodiment command parses");
        assert!(matches!(
            embodiment.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::LlmEmbodiment)
        ));

        let cross_host = Cli::try_parse_from(["xtask", "prove", "llm-cross-host"])
            .expect("cross-host LLM proof command parses");
        assert!(matches!(
            cross_host.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::LlmCrossHost)
        ));

        let local_model_pool = Cli::try_parse_from([
            "xtask",
            "prove",
            "local-model-pool",
            "--live-receipt",
            "live.json",
        ])
        .expect("local-model pool proof command parses");
        assert!(matches!(
            local_model_pool.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::LocalModelPool)
                && args.live_receipt.as_deref() == Some(std::path::Path::new("live.json"))
        ));

        let degraded = Cli::try_parse_from(["xtask", "prove", "degraded-profiles"])
            .expect("degraded-profile proof command parses");
        assert!(matches!(
            degraded.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::DegradedProfiles)
        ));

        let diversity =
            Cli::try_parse_from(["xtask", "prove", "diversity"]).expect("diversity proof parses");
        assert!(matches!(
            diversity.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::Diversity)
        ));

        let dormant = Cli::try_parse_from(["xtask", "prove", "dormant-readmission"])
            .expect("dormant-readmission proof parses");
        assert!(matches!(
            dormant.command,
            Command::Prove(args) if args.proof == Some(ProveTarget::DormantReadmission)
        ));

        let capture_restart = Cli::try_parse_from([
            "xtask",
            "prove",
            "browser-host",
            "--induce-capture-restart-failure",
        ])
        .expect("browser capture restart proof parses");
        assert!(matches!(
            capture_restart.command,
            Command::Prove(args) if args.induce_capture_restart_failure
        ));

        let pre_capture = Cli::try_parse_from([
            "xtask",
            "prove",
            "browser-host",
            "--induce-pre-capture-failure",
        ])
        .expect("browser pre-capture failure proof parses");
        assert!(matches!(
            pre_capture.command,
            Command::Prove(args) if args.induce_pre_capture_failure
        ));

        let proofs = Cli::try_parse_from(["xtask", "--json", "prove", "--list"])
            .expect("proof catalog command parses");
        assert!(proofs.global.json);
        assert!(matches!(
            proofs.command,
            Command::Prove(args) if args.list && args.proof.is_none()
        ));
        assert!(Cli::try_parse_from(["xtask", "proofs"]).is_err());

        let docs = Cli::try_parse_from(["xtask", "prove", "docs-verify", "--workspace-root", "."])
            .expect("proof evidence docs verifier parses");
        assert!(matches!(
            docs.command,
            Command::Prove(args)
                if matches!(
                    args.command,
                    Some(ProveCommand::Evidence(EvidenceCommand::DocsVerify(_)))
                )
        ));
        let verify = Cli::try_parse_from([
            "xtask",
            "prove",
            "verify",
            "--root",
            "target/evidence",
            "--commit",
            "0123456789012345678901234567890123456789",
            "--result",
            "complete",
        ])
        .expect("proof evidence verifier parses without an intermediate noun");
        assert!(matches!(
            verify.command,
            Command::Prove(args)
                if matches!(
                    args.command,
                    Some(ProveCommand::Evidence(EvidenceCommand::Verify(_)))
                )
        ));
        assert!(Cli::try_parse_from(["xtask", "prove", "evidence", "verify"]).is_err());
        assert!(Cli::try_parse_from(["xtask", "evidence", "docs-verify"]).is_err());

        let conduitos =
            Cli::try_parse_from(["xtask", "make", "conduitos", "prove", "--arch", "x86-64"])
                .expect("ConduitOS command parses");
        assert!(matches!(
            conduitos.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Conduitos(_)
            })
        ));

        let conduitos_evidence = Cli::try_parse_from([
            "xtask",
            "make",
            "conduitos",
            "prove",
            "--arch",
            "x86-64",
            "--evidence-root",
            "target/conduit-evidence/conduitos-x86_64",
        ])
        .expect("ConduitOS evidence command parses");
        assert!(matches!(
            conduitos_evidence.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Conduitos(_)
            })
        ));

        let audio =
            Cli::try_parse_from(["xtask", "doctor", "audio"]).expect("audio inspection parses");
        assert!(matches!(
            audio.command,
            Command::Doctor(DoctorArgs {
                target: DoctorTarget::Audio
            })
        ));
        let midi =
            Cli::try_parse_from(["xtask", "doctor", "midi"]).expect("MIDI inspection parses");
        assert!(matches!(
            midi.command,
            Command::Doctor(DoctorArgs {
                target: DoctorTarget::Midi
            })
        ));
        assert!(Cli::try_parse_from(["xtask", "audio", "list"]).is_err());
        assert!(Cli::try_parse_from(["xtask", "midi", "list"]).is_err());
        let pete = Cli::try_parse_from([
            "xtask",
            "prove",
            "pete",
            "std-observe",
            "--serial-path",
            "/dev/ttyUSB0",
            "--base-id",
            "std/create-uart/0",
            "--host-id",
            "std-host/0",
            "--boot-id",
            "std-boot/0",
            "--evidence-out",
            "target/pete-observation.json",
        ])
        .expect("explicit std Create observation entrance parses");
        assert!(matches!(
            pete.command,
            Command::Prove(args) if matches!(args.command, Some(ProveCommand::Pete(_)))
        ));
        let pete_workload = Cli::try_parse_from(["xtask", "check", "pete"])
            .expect("non-actuating Pete workload check entrance parses");
        assert!(matches!(pete_workload.command, Command::Check(_)));
        let pete_speaker = Cli::try_parse_from([
            "xtask",
            "prove",
            "pete",
            "std-speaker",
            "--serial-path",
            "/dev/ttyUSB0",
            "--base-id",
            "std/create-uart/0",
            "--host-id",
            "std-host/0",
            "--boot-id",
            "std-boot/0",
            "--robot-id",
            "robot/create1/0",
            "--attest-robot-identity",
            "--evidence-out",
            "target/pete-speaker.json",
        ])
        .expect("explicit std Create speaker entrance parses");
        assert!(matches!(pete_speaker.command, Command::Prove(_)));
        let pete_indicator = Cli::try_parse_from([
            "xtask",
            "prove",
            "pete",
            "std-indicator",
            "--serial-path",
            "/dev/ttyUSB0",
            "--base-id",
            "std/create-uart/0",
            "--host-id",
            "std-host/0",
            "--boot-id",
            "std-boot/0",
            "--robot-id",
            "robot/create1/0",
            "--attest-robot-identity",
            "--evidence-out",
            "target/pete-indicator.json",
        ])
        .expect("explicit std Create indicator entrance parses");
        assert!(matches!(pete_indicator.command, Command::Prove(_)));
        let pete_drive = Cli::try_parse_from([
            "xtask",
            "prove",
            "pete",
            "std-drive",
            "--serial-path",
            "/dev/ttyUSB0",
            "--base-id",
            "std/create-uart/0",
            "--host-id",
            "std-host/0",
            "--boot-id",
            "std-boot/0",
            "--robot-id",
            "robot/create1/0",
            "--attest-robot-identity",
            "--confirm-wheels-off-floor",
            "--evidence-out",
            "target/pete-drive.json",
        ])
        .expect("explicit std Create bounded drive entrance parses");
        assert!(matches!(pete_drive.command, Command::Prove(_)));
        assert!(Cli::try_parse_from(["xtask", "pete", "workload-check"]).is_err());
        assert!(Cli::try_parse_from([
            "xtask",
            "prove",
            "audio-playback",
            "--card-id",
            "PCH",
            "--device",
            "0",
        ])
        .is_err());
        Cli::try_parse_from([
            "xtask",
            "prove",
            "audio-playback",
            "--card-id",
            "PCH",
            "--device",
            "0",
            "--authorize-output",
        ])
        .expect("audio proof requires explicit output authority");

        let cue = Cli::try_parse_from([
            "xtask",
            "make",
            "startup-cue",
            "--output",
            "target/startup.wav",
        ])
        .expect("startup cue make parses");
        assert!(matches!(
            cue.command,
            Command::Make(MakeArgs {
                target: MakeTarget::StartupCue(_)
            })
        ));
    }
}
