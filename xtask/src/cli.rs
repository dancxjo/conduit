use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::commands::avr::AvrArgs;
use crate::commands::body::BodyArgs;
use crate::commands::body_coordination::BodyCoordinationArgs;
use crate::commands::catalog::CatalogArgs;
use crate::commands::check::CheckArgs;
use crate::commands::ci::CiArgs;
use crate::commands::conduitos::ConduitosArgs;
use crate::commands::esp32_firmware::Esp32FirmwareArgs;
use crate::commands::evidence::EvidenceArgs;
use crate::commands::forms::FormsArgs;
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
    /// Build or guardedly flash the Pete Pro Micro Brainstem.
    Avr(AvrArgs),
    /// Build and launch one independent browser page/WASM Host.
    Browser,
    /// Create, inspect, build, or deploy one whole Body description.
    Body(BodyArgs),
    /// Prove bounded Pete forebrain-motherbrain coordination.
    BodyCoordination(BodyCoordinationArgs),
    /// Inspect mechanically derived portable Kind coverage by Host profile.
    Catalog(CatalogArgs),
    /// Execute repository validation check suites.
    Check(CheckArgs),
    /// Plan repository CI obligations from an exact change.
    Ci(CiArgs),
    /// Execute platform and protocol proof suites.
    Prove(Box<ProveArgs>),
    /// Print the versioned machine-readable proof command contract.
    Proofs(ProofsArgs),
    /// Verify bounded proof evidence before transport or review.
    Evidence(EvidenceArgs),
    /// Check and report the explicit reviewed Form inventory.
    Forms(FormsArgs),
    /// Inspect repository and platform prerequisites.
    Doctor(DoctorArgs),
    /// Check the standalone ESP32 fabrication package without touching hardware.
    Esp32Firmware(Esp32FirmwareArgs),
    /// Build, flash, or verify the Pico W local Signal proof.
    Pico(PicoArgs),
    /// Target one Host lifecycle or manage exact Host configuration and fabrication.
    Host(HostArgs),
    /// Run the complete Pico W local workflow.
    PicoLocal(PicoArgs),
    /// Build and prove the freestanding ConduitOS reference Host.
    Conduitos(ConduitosArgs),
    /// Inspect and prove one explicit hosted PCM playback resource.
    Audio(AudioArgs),
    /// Inspect exact hosted MIDI sequencer endpoints.
    Midi(MidiArgs),
    /// Exercise explicit Pete development and hardware proof entrances.
    Pete(PeteArgs),
    /// Run interactive demonstrations.
    Demo(DemoArgs),
    /// Generate the bounded Patchbay GNU Unifont subset.
    UnifontSubset(UnifontSubsetArgs),
    /// Generate the bounded native masks for canonical palette icons.
    PaletteIcons(PaletteIconsArgs),
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
    pub proof: ProveTarget,

    /// Override the bounded evidence root for proofs that declare evidence outputs.
    #[arg(long)]
    pub evidence_root: Option<std::path::PathBuf>,

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

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothProofRole {
    Source,
    Sink,
}

#[derive(Args, Debug)]
pub struct ProofsArgs {
    /// Validate one JSON proof record against its exact registered command contract.
    #[arg(long)]
    pub validate_record: Option<std::path::PathBuf>,

    /// Run the one pinned finite proof-catalog validation obligation.
    #[arg(long)]
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
    DistributedLenia,
    DormantReadmission,
    RecursiveRecovery,
    LlmEmbodiment,
    LlmCrossHost,
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

#[derive(Args, Debug)]
pub struct DemoArgs {
    #[command(subcommand)]
    pub command: DemoCommand,
}

#[derive(Subcommand, Debug)]
pub enum DemoCommand {
    /// Open the executable Conduit Tour through the browser Host.
    Tour,
    /// Run the native Signal Form through the production kernel.
    Std,
    /// Run the three-sink Form entirely on the native Host.
    Triple,
    /// Build and launch the native Patchbay from this checkout.
    Patchbay(PatchbayDemoArgs),
    /// Let one Body BIRTH and open its canonical native Parts experience.
    BodyMembership,
    /// Open the authored physical-environment Patchbay demonstration.
    Environment,
    /// Rehearse a canonical Form against the authored environment before Wake.
    Prewake,
    /// Open the golden native Text Lab in effect-free PREWAKE, ready for the ordinary lifecycle.
    TextLab,
    /// Run the S4 distributed toggle proof interactively.
    Toggle,
    /// Run the attached C3-button/Pico synchronized physical light switch.
    LightSwitch(LightSwitchDemoArgs),
    /// Run the Conduit-driven project homepage interactively.
    Site,
    /// Run the pinned Tongues text-to-speech starter through an ordinary Conduit Play.
    Tongues,
    /// Run the bounded real-data Tongues paired-latent research capstone.
    TonguesResearch,
    /// Analyze the frozen Tongues latent dynamics with bounded controls.
    TonguesAnalysis,
}

#[derive(Args, Debug)]
pub struct LightSwitchDemoArgs {
    /// Stable USB-UART path for the inspected ESP32-C3 DevKitM-1.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Silicon_Labs_CP2102N_USB_to_UART_Bridge_Controller_dcf8355da19ded11a7205f84e259fb3e-if00-port0"
    )]
    pub c3_port: std::path::PathBuf,
    /// Stable CDC 0 command path for the light-switch Pico W image.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Conduit_Pico_W_Light_Switch_conduit-pico-w-light-switch-if00"
    )]
    pub pico_link_port: std::path::PathBuf,
    /// Stable CDC 1 receipt path for the light-switch Pico W image.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Conduit_Pico_W_Light_Switch_conduit-pico-w-light-switch-if02"
    )]
    pub pico_sign_port: std::path::PathBuf,
    /// Number of physical presses required before the bounded demo terminates.
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=16))]
    pub presses: u8,
}

#[derive(Args, Debug, Default)]
pub struct PatchbayDemoArgs {
    /// Select the Host realization used to manifest the shared entrance.
    #[arg(long, value_enum, default_value_t = PatchbayHost::Native)]
    pub on: PatchbayHost,
    /// Run the finite first-run authoring-to-Play acceptance journey.
    #[arg(long)]
    pub first_run_proof: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PatchbayHost {
    #[default]
    Native,
    Browser,
}

#[derive(Args, Debug)]
pub struct AudioArgs {
    #[command(subcommand)]
    pub command: AudioCommand,
}

#[derive(Subcommand, Debug)]
pub enum AudioCommand {
    /// List freshly observed ALSA playback resources without opening them.
    List,
    /// Run the bounded audible specimen through one exact selected output.
    PlaybackProof {
        /// Exact ALSA card identity from `cargo xtask audio list`.
        #[arg(long)]
        card_id: String,
        /// Exact ALSA device number on the selected card.
        #[arg(long)]
        device: u16,
        /// Explicitly authorize sounding the selected output for this proof.
        #[arg(long, required = true)]
        authorize_output: bool,
    },
}

#[derive(Args, Debug)]
pub struct MidiArgs {
    #[command(subcommand)]
    pub command: MidiCommand,
}

#[derive(Subcommand, Debug)]
pub enum MidiCommand {
    /// List fresh directional ALSA sequencer metadata without opening a port.
    List,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// What to inspect (default: all).
    #[arg(default_value = "all")]
    pub target: DoctorTarget,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorTarget {
    All,
    Browser,
    Pico,
}

impl DoctorTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Browser => "browser",
            Self::Pico => "pico",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_and_pico_commands_parse() {
        let matrix = Cli::try_parse_from(["xtask", "catalog", "matrix"])
            .expect("catalog matrix command parses");
        assert!(matches!(matrix.command, Command::Catalog(_)));

        let gap = Cli::try_parse_from(["xtask", "catalog", "gap", "--host", "pico"])
            .expect("catalog gap command parses");
        assert!(matches!(gap.command, Command::Catalog(_)));

        let doctor = Cli::try_parse_from(["xtask", "--dry-run", "doctor", "pico"])
            .expect("doctor command parses");
        assert!(doctor.global.dry_run);
        assert!(matches!(doctor.command, Command::Doctor(_)));

        let pico = Cli::try_parse_from(["xtask", "pico", "build"]).expect("pico command parses");
        assert!(matches!(pico.command, Command::Pico(_)));

        let host = Cli::try_parse_from([
            "xtask",
            "host",
            "build",
            "profile.json",
            "--output",
            "target/host-image",
            "--source-identity",
            "git:abc",
        ])
        .expect("host BUILD command parses");
        assert!(matches!(host.command, Command::Host(_)));

        let body_new = Cli::try_parse_from([
            "xtask",
            "body",
            "new",
            "pete",
            "--template",
            "robot",
            "--host",
            "eyes=browser-page",
        ])
        .expect("Body scaffold command parses");
        assert!(matches!(body_new.command, Command::Body(_)));
        assert!(
            Cli::try_parse_from(["xtask", "body", "new", "pete", "--template", "unknown"]).is_err()
        );
        let guided_body = Cli::try_parse_from(["xtask", "body", "new"])
            .expect("interactive Body scaffold may prompt for its name");
        assert!(matches!(guided_body.command, Command::Body(_)));
        let scripted_body =
            Cli::try_parse_from(["xtask", "body", "new", "pete", "--no-interactive"])
                .expect("scripted Body scaffold parses");
        assert!(matches!(scripted_body.command, Command::Body(_)));

        for command in [
            vec!["xtask", "host"],
            vec!["xtask", "host", "std"],
            vec!["xtask", "host", "browser"],
            vec!["xtask", "host", "rpi"],
            vec!["xtask", "host", "rpi", "--board", "rpi-zero-v1", "image"],
            vec![
                "xtask",
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
                "host",
                "rpi",
                "physical-proof",
                "--serial-device",
                "/dev/ttyUSB0",
            ],
        ] {
            let parsed = Cli::try_parse_from(command.clone())
                .unwrap_or_else(|error| panic!("host command {command:?} must parse: {error}"));
            assert!(matches!(parsed.command, Command::Host(_)));
        }
        assert!(
            Cli::try_parse_from(["xtask", "host", "rpi", "flash", "--device", "/dev/sda",])
                .is_err()
        );
        assert!(Cli::try_parse_from(["xtask", "host", "rpi", "--board", "rpi-5",]).is_err());

        let pico_body = Cli::try_parse_from([
            "xtask",
            "pico",
            "prove-body-admission",
            "--link-port",
            "/dev/serial/by-id/pico",
        ])
        .expect("physical Pico Body admission proof parses");
        assert!(matches!(
            pico_body.command,
            Command::Pico(PicoArgs {
                subcommand: Some(crate::commands::pico::PicoSubcommand::ProveBodyAdmission),
                ..
            })
        ));

        let pico_build_remote = Cli::try_parse_from(["xtask", "pico", "build", "--usb-remote"])
            .expect("pico build --usb-remote parses");
        if let Command::Pico(args) = pico_build_remote.command {
            assert!(args.usb_remote);
        } else {
            panic!("expected Command::Pico");
        }

        let pico_flash_remote = Cli::try_parse_from(["xtask", "pico", "flash", "--usb-remote"])
            .expect("pico flash --usb-remote parses");
        if let Command::Pico(args) = pico_flash_remote.command {
            assert!(args.usb_remote);
        } else {
            panic!("expected Command::Pico");
        }

        let pico_build_control = Cli::try_parse_from(["xtask", "pico", "build", "--r1-control"])
            .expect("pico build --r1-control parses");
        if let Command::Pico(args) = pico_build_control.command {
            assert!(args.r1_control);
        } else {
            panic!("expected Command::Pico");
        }

        let toggle =
            Cli::try_parse_from(["xtask", "demo", "toggle"]).expect("demo toggle command parses");
        assert!(matches!(
            toggle.command,
            Command::Demo(DemoArgs {
                command: DemoCommand::Toggle
            })
        ));

        for command in ["tour", "std", "triple", "patchbay", "body-membership"] {
            Cli::try_parse_from(["xtask", "demo", command])
                .unwrap_or_else(|error| panic!("demo {command} must parse: {error}"));
        }
        let browser = Cli::try_parse_from(["xtask", "browser"]).expect("browser Host parses");
        assert!(matches!(browser.command, Command::Browser));
        assert!(Cli::try_parse_from(["xtask", "demo", "browser"]).is_err());

        let site =
            Cli::try_parse_from(["xtask", "demo", "site"]).expect("demo site command parses");
        assert!(matches!(
            site.command,
            Command::Demo(DemoArgs {
                command: DemoCommand::Site
            })
        ));

        let subset =
            Cli::try_parse_from(["xtask", "unifont-subset", "unifont.hex.gz", "subset.hex"])
                .expect("unifont-subset command parses");
        assert!(matches!(subset.command, Command::UnifontSubset(_)));

        let icons = Cli::try_parse_from([
            "xtask",
            "palette-icons",
            "assets/icons/lucide/svg",
            "icons.rs",
        ])
        .expect("palette-icons command parses");
        assert!(matches!(icons.command, Command::PaletteIcons(_)));

        let check =
            Cli::try_parse_from(["xtask", "check", "workspace"]).expect("check command parses");
        assert!(matches!(check.command, Command::Check(_)));

        let ci = Cli::try_parse_from([
            "xtask",
            "ci",
            "plan",
            "0123456789012345678901234567890123456789",
            "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
        ])
        .expect("CI impact plan command parses");
        assert!(matches!(ci.command, Command::Ci(_)));

        let prove = Cli::try_parse_from(["xtask", "prove", "std-browser-s4"])
            .expect("prove command parses");
        assert!(matches!(prove.command, Command::Prove(_)));

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
            Command::Prove(args) if args.proof == ProveTarget::CalendarGoogle
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
            Command::Prove(args) if args.proof == ProveTarget::MessagingGithub
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
            Command::Prove(args) if args.proof == ProveTarget::LlmPlanningAdvice
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
            Command::Prove(args) if args.proof == ProveTarget::LlmEmbodiment
        ));

        let cross_host = Cli::try_parse_from(["xtask", "prove", "llm-cross-host"])
            .expect("cross-Host LLM proof command parses");
        assert!(matches!(
            cross_host.command,
            Command::Prove(args) if args.proof == ProveTarget::LlmCrossHost
        ));

        let degraded = Cli::try_parse_from(["xtask", "prove", "degraded-profiles"])
            .expect("degraded-profile proof command parses");
        assert!(matches!(
            degraded.command,
            Command::Prove(args) if args.proof == ProveTarget::DegradedProfiles
        ));

        let diversity =
            Cli::try_parse_from(["xtask", "prove", "diversity"]).expect("diversity proof parses");
        assert!(matches!(
            diversity.command,
            Command::Prove(args) if args.proof == ProveTarget::Diversity
        ));

        let dormant = Cli::try_parse_from(["xtask", "prove", "dormant-readmission"])
            .expect("dormant-readmission proof parses");
        assert!(matches!(
            dormant.command,
            Command::Prove(args) if args.proof == ProveTarget::DormantReadmission
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

        let proofs = Cli::try_parse_from(["xtask", "--json", "proofs"])
            .expect("proof catalog command parses");
        assert!(proofs.global.json);
        assert!(matches!(proofs.command, Command::Proofs(_)));

        let docs =
            Cli::try_parse_from(["xtask", "evidence", "docs-verify", "--workspace-root", "."])
                .expect("evidence docs verifier parses");
        assert!(matches!(docs.command, Command::Evidence(_)));

        let conduitos = Cli::try_parse_from(["xtask", "conduitos", "prove", "--arch", "x86-64"])
            .expect("ConduitOS command parses");
        assert!(matches!(conduitos.command, Command::Conduitos(_)));

        let conduitos_evidence = Cli::try_parse_from([
            "xtask",
            "conduitos",
            "prove",
            "--arch",
            "x86-64",
            "--evidence-root",
            "target/conduit-evidence/conduitos-x86_64",
        ])
        .expect("ConduitOS evidence command parses");
        assert!(matches!(conduitos_evidence.command, Command::Conduitos(_)));

        let audio = Cli::try_parse_from(["xtask", "audio", "list"])
            .expect("audio discovery command parses");
        assert!(matches!(audio.command, Command::Audio(_)));
        let midi =
            Cli::try_parse_from(["xtask", "midi", "list"]).expect("MIDI discovery command parses");
        assert!(matches!(midi.command, Command::Midi(_)));
        let pete = Cli::try_parse_from([
            "xtask",
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
        assert!(matches!(pete.command, Command::Pete(_)));
        let pete_speaker = Cli::try_parse_from([
            "xtask",
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
        assert!(matches!(pete_speaker.command, Command::Pete(_)));
        let pete_indicator = Cli::try_parse_from([
            "xtask",
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
        assert!(matches!(pete_indicator.command, Command::Pete(_)));
        let pete_drive = Cli::try_parse_from([
            "xtask",
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
        assert!(matches!(pete_drive.command, Command::Pete(_)));
        assert!(Cli::try_parse_from([
            "xtask",
            "audio",
            "playback-proof",
            "--card-id",
            "PCH",
            "--device",
            "0",
        ])
        .is_err());
        Cli::try_parse_from([
            "xtask",
            "audio",
            "playback-proof",
            "--card-id",
            "PCH",
            "--device",
            "0",
            "--authorize-output",
        ])
        .expect("audio proof requires explicit output authority");
    }
}
