use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Product command-line entrance for installed Conduit workflows.
#[derive(Debug, Parser)]
#[command(
    name = "conduit",
    about = "Enter Conduit, run Plots, and inspect exact truth",
    long_about = "Enter Conduit, run Plots, and inspect exact truth.\n\nWith no command, Conduit enters the current Body named by CONDUIT_STATE_DIR. If that Host does not yet belong to a Body, Conduit enters the birth encounter instead."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Check, plan, admit, and execute a plot on available local hosts.
    Run {
        /// Authored plot to execute.
        plot: PathBuf,
        /// Optional exact placement constraints.
        #[arg(long)]
        placements: Option<PathBuf>,
        /// Write a neutral runtime report after execution.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Retain separately validated Plan, Play, and Sign artifacts in a new directory.
        #[arg(long)]
        artifacts: Option<PathBuf>,
        /// Exact canonical Body construction source used for Host and Line truth.
        #[arg(long)]
        body: Option<PathBuf>,
        /// Do not attach standard-input cancellation; wait for the play's own terminal outcome.
        #[arg(long)]
        await_terminal: bool,
    },
    /// Inspect this Host or perform one installed Host action.
    Host {
        #[command(subcommand)]
        command: Option<HostCommand>,
    },
    /// Inspect this Body or perform one durable Body lifecycle operation.
    Body {
        #[command(subcommand)]
        command: Option<BodyCommand>,
    },
    /// Check a plot and render owned diagnostics without executing it.
    Check {
        plot: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Explain concise source as the ordinary checked semantics it names.
    Expand {
        plot: PathBuf,
        /// Emit the structured source-correlation model for editor tooling.
        #[arg(long)]
        json: bool,
    },
    /// Render a checked plot's gears, ports, and exact cords.
    Diagram {
        /// Authored plot to visualize. The last public plot is rendered.
        plot: PathBuf,
        /// Diagram representation to emit.
        #[arg(long, value_enum, default_value_t = DiagramFormat::Svg)]
        format: DiagramFormat,
        /// Write the diagram to this path instead of standard output.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Inspect retained Conduit artifacts without executing work.
    Inspect {
        /// Artifact whose schema identifies the truth to render.
        thing: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum DiagramFormat {
    /// Standalone SVG requiring no external renderer.
    Svg,
    /// Mermaid flowchart source for embedding in supporting tools.
    Mermaid,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HostCommand {
    /// Install one exact reviewed target, or inspect its plan with --dry-run.
    Install {
        target: String,
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        catalog_id: String,
        #[arg(long)]
        mirror: PathBuf,
        #[arg(long)]
        cache: PathBuf,
        #[arg(long = "carrier-descriptor", required = true, num_args = 1..)]
        carrier_descriptors: Vec<PathBuf>,
        #[arg(long)]
        carrier: Option<String>,
        #[arg(long, default_value_t = 0)]
        minimum_generation: u64,
        /// Bounded carrier-specific installation request. Required unless --dry-run.
        #[arg(long, required_unless_present = "dry_run")]
        request: Option<PathBuf>,
        /// Inspect exact stages and authority without performing carrier effects.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        dry_run: bool,
    },
    /// Install, run, or inspect the durable local host owner.
    Service {
        #[command(subcommand)]
        command: HostServiceCommand,
    },
    /// Obtain one target's reviewed release manifest from HTTPS or an offline mirror.
    #[command(hide = true)]
    Obtain {
        /// Reviewed target identity to obtain.
        target: String,
        /// Bounded catalog path or HTTPS URL.
        #[arg(long)]
        catalog: PathBuf,
        /// Exact expected catalog identity from the installed release channel.
        #[arg(long)]
        catalog_id: String,
        /// Local/air-gapped mirror root or HTTPS base URL.
        #[arg(long)]
        mirror: PathBuf,
        /// Immutable content-addressed cache directory.
        #[arg(long)]
        cache: PathBuf,
        /// Refuse catalogs at or below this generation.
        #[arg(long, default_value_t = 0)]
        minimum_generation: u64,
    },
    /// Carry an exact body-bound artifact without rebuilding it.
    #[command(hide = true)]
    Carry {
        #[command(subcommand)]
        command: CarrierCommand,
    },
    /// Offer this already-running Host to an attended Body invitation.
    Rendezvous {
        /// Installed durable host state owned by the running service.
        #[arg(long)]
        state_dir: PathBuf,
        /// Line carrier used to reach this running host.
        #[arg(long, value_enum, default_value_t = RendezvousCarrier::Websocket)]
        carrier: RendezvousCarrier,
        /// Stop a WebSocket carrier if the code is unused for this many seconds.
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=3600))]
        timeout_seconds: u64,
        /// Exact non-loopback socket to expose for the secure WebSocket carrier.
        #[arg(long, required_if_eq("carrier", "secure-websocket"))]
        bind: Option<String>,
        /// Browser-reachable wss URL whose host is covered by the TLS certificate.
        #[arg(long, required_if_eq("carrier", "secure-websocket"))]
        public_url: Option<String>,
        /// PEM certificate chain for the explicitly exposed TLS identity.
        #[arg(long, required_if_eq("carrier", "secure-websocket"))]
        tls_cert: Option<PathBuf>,
        /// PEM private key for the explicitly exposed TLS identity.
        #[arg(long, required_if_eq("carrier", "secure-websocket"))]
        tls_key: Option<PathBuf>,
        /// Private endpoint descriptor for one outbound user-operated relay candidate.
        #[arg(long, required_if_eq("carrier", "relay"))]
        relay_descriptor: Option<PathBuf>,
        /// Explicitly authorize listening beyond loopback.
        #[arg(long, required_if_eq("carrier", "secure-websocket"), action = clap::ArgAction::SetTrue)]
        authorize_network: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CarrierCommand {
    /// Report the exact reviewed carriers available for one selected target.
    Availability {
        /// One or more reviewed carrier descriptor documents for the selected target.
        #[arg(required = true, num_args = 1..)]
        descriptors: Vec<PathBuf>,
    },
    /// Copy the exact artifact without starting or writing a host.
    Download {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        destination: PathBuf,
    },
    /// Install and start an exact body-bound native Host package locally.
    InstallNative {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        /// Durable local host state retained across service restarts.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize installation and service start on this machine.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_install: bool,
    },
    /// Flash an exact body-bound RP2040 UF2 to one confirmed BOOTSEL volume.
    FlashUf2 {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        /// Mounted RP2040 BOOTSEL volume containing INFO_UF2.TXT.
        volume: PathBuf,
        /// Repeat the exact mounted volume path to prevent ambiguous device selection.
        #[arg(long)]
        confirm_volume: String,
        /// Explicitly authorize writing firmware to the confirmed microcontroller.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_flash: bool,
    },
    /// Launch the exact artifact through the reviewed local VM profile.
    LaunchVm {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_launch: bool,
    },
    /// Serve an exact body-bound boot image over finite HTTP Boot.
    ServeHttpBoot {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        /// Explicit network address to bind, such as 0.0.0.0:8080.
        #[arg(long)]
        bind: String,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=64))]
        maximum_requests: u16,
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=3600))]
        timeout_seconds: u64,
        /// Explicitly authorize exposing this exact boot artifact on the selected network address.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_serve: bool,
    },
    /// Destructively write and verify one explicitly confirmed removable device.
    WriteRemovable {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        destination: PathBuf,
        #[arg(long)]
        confirm_destination: String,
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_write: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RendezvousCarrier {
    /// Local browser WebSocket Line.
    Websocket,
    /// Explicit TLS-authenticated WebSocket Line on a selected network endpoint.
    SecureWebsocket,
    /// Newline-framed serial stream on standard input and output.
    Serial,
    /// Outbound end-to-end protected Line through a user-operated relay.
    Relay,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HostServiceCommand {
    /// Verify and install one reviewed release bundle without replacing durable identity.
    Install {
        manifest: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
        /// Install without activating the service, for foreground Body ownership.
        #[arg(long)]
        no_start: bool,
        #[command(flatten)]
        speech: InstalledSpeechOptions,
    },
    /// Run the durable host in the foreground for a platform service manager.
    Run {
        #[arg(long)]
        state_dir: PathBuf,
    },
    /// Print the retained host identity and current runtime status.
    Status {
        #[arg(long)]
        state_dir: PathBuf,
        /// Emit bounded machine-readable Host, Boot, and release identity truth.
        #[arg(long)]
        json: bool,
    },
    /// Make this durable host retain one exact validated Body biography.
    #[command(hide = true)]
    OwnBody {
        /// Exported `conduit.body/biography-evidence@2` document.
        evidence: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
    },
}

/// Exact local equipment selected for every fresh installed Host Boot.
#[derive(Debug, Default, Args)]
pub(crate) struct InstalledSpeechOptions {
    /// Select a currently observed speaker and verified eSpeak NG provider.
    #[arg(long, requires_all = ["speaker_card", "speaker_device", "speech_executable", "speech_data", "speech_engine"])]
    pub(crate) selected_speech: bool,
    /// Remove a previously retained speech selection on reinstall.
    #[arg(long, conflicts_with = "selected_speech")]
    pub(crate) without_selected_speech: bool,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speaker_card: Option<String>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speaker_device: Option<u16>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speech_executable: Option<PathBuf>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speech_data: Option<PathBuf>,
    #[arg(long, requires = "selected_speech", num_args = 1..)]
    pub(crate) speech_engine: Vec<PathBuf>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speech_voice: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BodyCommand {
    /// List current selectable local speakers and verified speech providers.
    SpeechOptions {
        /// Emit a bounded machine-readable observation; opens no speaker.
        #[arg(long)]
        json: bool,
    },
    /// Enter the birth encounter for a Host that does not yet belong to a Body.
    Birth {
        /// Use a terminal and nonvisual command input for the zero-Body Crèche.
        #[arg(long)]
        screen_free: bool,
        /// The installed Host whose current Boot owns the Birth encounter.
        #[arg(long)]
        state_dir: Option<PathBuf>,
        #[command(flatten)]
        speech: BirthSpeechOptions,
    },
    /// Read the installed owner's exact current Body Face and Host advertisement.
    Face {
        #[arg(long)]
        state_dir: PathBuf,
        /// Emit the bounded machine-readable Face snapshot envelope.
        #[arg(long)]
        json: bool,
    },
    /// Own a retained Body on an installed Linux Host in the foreground.
    ///
    /// Run the installed product executable with its service stopped. Standard
    /// input/output carry bounded internal JSON control, not a terminal Mask.
    Own {
        /// Checked canonical Plot source; must match retained source on recovery.
        source: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long, default_value = "My Body")]
        name: String,
    },
    /// Inspect the body retained by this installed host.
    #[command(hide = true)]
    Status {
        /// Installed durable host state that owns or has joined the body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Emit bounded machine-readable Body, Host, Boot, and biography identity truth.
        #[arg(long)]
        json: bool,
    },
    /// Start the retained Plot on the installed owner's current Host Boot.
    Start {
        #[arg(long)]
        state_dir: PathBuf,
        /// Finite execution deadline; the Play can be lulled earlier.
        #[arg(long, default_value_t = 300_000, value_parser = clap::value_parser!(u64).range(1..=900_000))]
        maximum_millis: u64,
    },
    /// Request that the current service-owned Body Play stop and lull.
    Lull {
        #[arg(long)]
        state_dir: PathBuf,
    },
    /// Read the installed owner's live Face through the terminal Mask.
    Terminal {
        #[arg(long)]
        state_dir: PathBuf,
        /// Attach this terminal to the owner Host for an acknowledged Show and typed action.
        #[arg(long)]
        owner_show: bool,
    },
    /// Issue one bounded invitation from the body owned by this installed host.
    Invite {
        /// Installed durable host state that owns the body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Invitation lifetime; never exceeds the architectural maximum.
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=600))]
        ttl_seconds: u64,
        /// Exact network socket for one finite authenticated owner route.
        #[arg(long)]
        route_bind: Option<std::net::SocketAddr>,
        /// Public `wss://` URL carried by the routed invitation.
        #[arg(long)]
        route_url: Option<String>,
        /// TLS certificate whose exact leaf digest authenticates the owner endpoint.
        #[arg(long)]
        route_tls_cert: Option<PathBuf>,
        /// TLS private key for the finite owner route.
        #[arg(long)]
        route_tls_key: Option<PathBuf>,
        /// Explicitly authorize exposing this one-invitation admission route.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        authorize_route: bool,
    },
    /// Open one short-lived local window for this browser Host to join the installed owner.
    BrowserWindow {
        /// Installed Host state whose running service owns the Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Exact Host ID shown by the browser's "Join an owned Body" view.
        #[arg(long)]
        expected_host_id: String,
        /// JSON byte array copied from the browser for first admission; omit on return.
        #[arg(long)]
        new_host_verifying_key: Option<String>,
        /// Finite admission and presence window, at most one minute.
        #[arg(long, default_value_t = 60_000, value_parser = clap::value_parser!(u64).range(1000..=60_000))]
        maximum_millis: u64,
        /// Explicitly authorize the local loopback browser admission window.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_window: bool,
    },
    /// Join the Body named by one routed portable invitation.
    Join {
        /// Routed invitation JSON path, or `-` to read it from standard input.
        invitation: PathBuf,
        /// Installed durable host state that will join the invited Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize this Host to request and retain Body membership.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_join: bool,
    },
    /// Accept one bounded invitation on this installed host and emit an admission request.
    #[command(hide = true)]
    Accept {
        /// Invitation JSON path, or `-` to read the exact document from standard input.
        invitation: PathBuf,
        /// Installed durable host state that will join the invited Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize this host to request membership in the invited Body.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_join: bool,
    },
    /// Admit one signed request into the body owned by this installed host.
    #[command(hide = true)]
    Admit {
        /// Admission-request JSON path, or `-` to read the exact document from standard input.
        request: PathBuf,
        /// Installed durable host state that owns the body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize adding the requested Host as a body Part.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_admission: bool,
    },
    /// Retain the owner's exact admission receipt as this host's durable membership.
    #[command(hide = true)]
    CompleteJoin {
        /// Owner-issued `conduit.body/spawn-admission-receipt@1` document.
        receipt: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize retaining membership in the admitted body.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_membership: bool,
    },
}

/// Explicit local synthesis and speaker selection for a screen-free Birth.
#[derive(Debug, Default, Args)]
pub(crate) struct BirthSpeechOptions {
    /// Speak through one selected, currently discovered ALSA speaker.
    #[arg(long, requires_all = ["screen_free", "speaker_card", "speaker_device", "speech_executable", "speech_data", "speech_engine"])]
    pub(crate) speak: bool,
    /// ALSA card ID from `conduit body speech-options`.
    #[arg(long, requires = "speak")]
    pub(crate) speaker_card: Option<String>,
    /// ALSA device number on the selected card.
    #[arg(long, requires = "speak")]
    pub(crate) speaker_device: Option<u16>,
    /// Exact installed eSpeak NG executable.
    #[arg(long, requires = "speak")]
    pub(crate) speech_executable: Option<PathBuf>,
    /// Exact installed espeak-ng-data directory.
    #[arg(long, requires = "speak")]
    pub(crate) speech_data: Option<PathBuf>,
    /// Exact eSpeak NG engine library and any same-directory dependencies.
    #[arg(long, requires = "speak", num_args = 1..)]
    pub(crate) speech_engine: Vec<PathBuf>,
    /// Voice within the selected installed data tree (default: en-us).
    #[arg(long, requires = "speak")]
    pub(crate) speech_voice: Option<String>,
}

#[cfg(test)]
mod public_surface_tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn no_arguments_enters_conduit() {
        assert!(Cli::try_parse_from(["conduit"])
            .expect("bare product entrance parses")
            .command
            .is_none());
    }

    #[test]
    fn screen_free_birth_is_an_explicit_product_entrance() {
        assert!(matches!(
            Cli::try_parse_from(["conduit", "body", "birth", "--screen-free"])
                .expect("screen-free Birth parses")
                .command,
            Some(Command::Body {
                command: Some(BodyCommand::Birth {
                    screen_free: true,
                    ..
                })
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "body", "birth"])
                .expect("browser Birth remains default")
                .command,
            Some(Command::Body {
                command: Some(BodyCommand::Birth {
                    screen_free: false,
                    ..
                })
            })
        ));
    }

    #[test]
    fn speaker_output_requires_one_explicit_device_and_provider() {
        assert!(matches!(
            Cli::try_parse_from(["conduit", "body", "speech-options", "--json"])
                .unwrap()
                .command,
            Some(Command::Body {
                command: Some(BodyCommand::SpeechOptions { json: true })
            })
        ));
        assert!(
            Cli::try_parse_from(["conduit", "body", "birth", "--screen-free", "--speak"]).is_err()
        );
        assert!(Cli::try_parse_from([
            "conduit",
            "body",
            "birth",
            "--screen-free",
            "--speaker-card",
            "sofhdadsp",
        ])
        .is_err());
        let selected = Cli::try_parse_from([
            "conduit",
            "body",
            "birth",
            "--screen-free",
            "--speak",
            "--speaker-card",
            "sofhdadsp",
            "--speaker-device",
            "0",
            "--speech-executable",
            "/usr/bin/espeak-ng",
            "--speech-data",
            "/usr/lib/espeak-ng-data",
            "--speech-engine",
            "/usr/lib/libespeak-ng.so.1",
        ])
        .unwrap();
        assert!(matches!(
            selected.command,
            Some(Command::Body {
                command: Some(BodyCommand::Birth {
                    speech: BirthSpeechOptions { speak: true, .. },
                    ..
                })
            })
        ));
    }

    #[test]
    fn public_help_names_intent_not_retired_shells_or_protocol_phases() {
        let help = Cli::command().render_long_help().to_string();
        for entrance in [
            "run", "check", "expand", "diagram", "inspect", "body", "host",
        ] {
            assert!(help.contains(entrance), "missing {entrance} in:\n{help}");
        }
        for retired in ["creche", "patchbay", "copy", "rendezvous-relay"] {
            assert!(
                !help.contains(retired),
                "retired {retired} remains in:\n{help}"
            );
        }
    }

    #[test]
    fn body_host_and_schema_driven_inspection_parse_without_status_ceremony() {
        assert!(matches!(
            Cli::try_parse_from(["conduit", "body"])
                .expect("bare body inspection parses")
                .command,
            Some(Command::Body { command: None })
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "host"])
                .expect("bare host inspection parses")
                .command,
            Some(Command::Host { command: None })
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "inspect", "run.json"])
                .expect("artifact inspection parses")
                .command,
            Some(Command::Inspect { thing }) if thing == std::path::Path::new("run.json")
        ));
    }

    #[test]
    fn body_help_exposes_joining_intent_without_protocol_phases() {
        let mut body = Cli::command()
            .find_subcommand("body")
            .expect("body command")
            .clone();
        let help = body.render_long_help().to_string();
        for entrance in ["birth", "invite", "join"] {
            assert!(help.contains(entrance), "missing {entrance} in:\n{help}");
        }
        for phase in ["accept", "admit", "complete-join", "status"] {
            assert!(
                !help.contains(&format!("\n  {phase}")),
                "hidden phase {phase} leaked in:\n{help}"
            );
        }
    }

    #[test]
    fn source_expansion_is_a_public_human_and_machine_entrance() {
        assert!(matches!(
            Cli::try_parse_from(["conduit", "expand", "example.conduit"])
                .expect("human source expansion parses")
                .command,
            Some(Command::Expand { plot, json: false }) if plot == std::path::Path::new("example.conduit")
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "expand", "example.conduit", "--json"])
                .expect("machine source expansion parses")
                .command,
            Some(Command::Expand { json: true, .. })
        ));
    }

    #[test]
    fn plot_diagram_defaults_to_svg_and_accepts_mermaid() {
        assert!(matches!(
            Cli::try_parse_from(["conduit", "diagram", "example.conduit"])
                .expect("diagram parses")
                .command,
            Some(Command::Diagram {
                format: DiagramFormat::Svg,
                output: None,
                ..
            })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "diagram",
                "example.conduit",
                "--format",
                "mermaid",
                "--output",
                "example.mmd",
            ])
            .expect("Mermaid diagram parses")
            .command,
            Some(Command::Diagram {
                format: DiagramFormat::Mermaid,
                output: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn host_install_and_service_are_public_while_protocol_stages_stay_hidden() {
        let host_help = HostCommand::augment_subcommands(clap::Command::new("host"))
            .render_long_help()
            .to_string();
        assert!(host_help.contains("install"));
        assert!(host_help.contains("service"));
        for hidden in ["obtain", "carry"] {
            assert!(
                !host_help.contains(hidden),
                "hidden {hidden} leaked in:\n{host_help}"
            );
        }
        let service_help = HostServiceCommand::augment_subcommands(clap::Command::new("service"))
            .render_long_help()
            .to_string();
        assert!(service_help.contains("install"));
        assert!(!service_help.contains("own-body"));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "install",
                "hosted/linux",
                "--catalog",
                "catalog.json",
                "--catalog-id",
                "stable",
                "--mirror",
                "mirror",
                "--cache",
                "cache",
                "--carrier-descriptor",
                "native.json",
                "--dry-run",
            ])
            .expect("host install dry-run parses")
            .command,
            Some(Command::Host {
                command: Some(HostCommand::Install { dry_run: true, .. })
            })
        ));
    }
    #[test]
    fn service_install_only_skips_start_when_explicitly_requested() {
        for (extra, expected) in [(None, false), (Some("--no-start"), true)] {
            let mut args = vec![
                "conduit",
                "host",
                "service",
                "install",
                "release.json",
                "--state-dir",
                "state",
            ];
            args.extend(extra);
            assert!(matches!(Cli::try_parse_from(args).unwrap().command,
                Some(Command::Host { command: Some(HostCommand::Service {
                    command: HostServiceCommand::Install { no_start, .. }
                }) }) if no_start == expected));
        }
    }

    #[test]
    fn service_install_selection_requires_complete_explicit_equipment() {
        let base = [
            "conduit",
            "host",
            "service",
            "install",
            "release.json",
            "--state-dir",
            "state",
        ];
        assert!(Cli::try_parse_from(base.iter().copied().chain(["--selected-speech"])).is_err());
        assert!(
            Cli::try_parse_from(base.iter().copied().chain(["--speaker-card", "card"])).is_err()
        );
        let selected = base.iter().copied().chain([
            "--selected-speech",
            "--speaker-card",
            "card",
            "--speaker-device",
            "0",
            "--speech-executable",
            "/bin/espeak-ng",
            "--speech-data",
            "/data/espeak-ng-data",
            "--speech-engine",
            "/lib/libespeak-ng.so.1",
        ]);
        assert!(matches!(
            Cli::try_parse_from(selected).unwrap().command,
            Some(Command::Host {
                command: Some(HostCommand::Service {
                    command: HostServiceCommand::Install {
                        speech: InstalledSpeechOptions {
                            selected_speech: true,
                            ..
                        },
                        ..
                    }
                })
            })
        ));
        assert!(Cli::try_parse_from(
            base.iter()
                .copied()
                .chain(["--without-selected-speech", "--selected-speech"])
        )
        .is_err());
    }
}
