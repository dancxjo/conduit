use clap::{Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
use std::path::PathBuf;

/// Product command-line entrance for installed Conduit workflows.
#[derive(Debug, Parser)]
#[command(
    name = "conduit",
    about = "Run Forms and grow a living Conduit Body",
    long_about = "Run Forms and grow a living Conduit Body.\n\nUse `conduit body invite` to issue bounded joining authority, `conduit body accept` on an installed machine to prepare its signed admission request, and `conduit body admit` on the owning machine to commit membership and current presence. Use `conduit host obtain` to resolve a reviewed target release. Body binding remains separate from `conduit host carry`, which downloads, launches, writes, or flashes an exact artifact through an explicit carrier. Inspect durable identity and current runtime truth with `conduit host service status`.",
    after_help = "BODY GROWTH\n  1. conduit body invite --state-dir <OWNER_STATE> > invitation.json\n  2. conduit body accept invitation.json --state-dir <JOINING_STATE> --authorize-join > request.json\n  3. conduit body admit request.json --state-dir <OWNER_STATE> --authorize-admission > receipt.json\n  4. conduit body complete-join receipt.json --state-dir <JOINING_STATE> --authorize-membership\n  5. conduit host obtain <TARGET> --catalog <CATALOG> --catalog-id <ID> --mirror <MIRROR> --cache <CACHE>\n  6. conduit host carry <CARRIER> --help\n\nInvitation, request, and receipt documents are bounded JSON suitable for standard input/output. Admission reports membership and current offers without creating a Plan or Play. Artifact preparation never implies carrier execution, boot, admission, Plan, or Play."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Enter Conduit Home through the native presentation available on this Host.
    Home,
    /// Birth and provision a Body through the browser Crèche.
    Creche,
    /// Enter the current Body through the shared Patchbay front door.
    Patchbay {
        /// Select the Host realization used to manifest Patchbay.
        #[arg(long, value_enum, default_value_t = PatchbayHost::Native)]
        on: PatchbayHost,
        /// Open exact exported Body biography evidence in an external reader.
        #[arg(long)]
        body_evidence: Option<PathBuf>,
        /// Offer one explicit local Body invitation to this browser Host.
        #[arg(long, requires = "body_evidence")]
        body_invitation: Option<String>,
        /// Admit a reviewed Form label and canonical source for Body workload changes.
        #[arg(long, value_names = ["LABEL", "PATH"], num_args = 2, action = clap::ArgAction::Append, requires = "body_evidence")]
        reviewed_form: Vec<OsString>,
    },
    /// Check, plan, admit, and execute a Form on available local Hosts.
    Run {
        /// Authored Form to execute.
        form: PathBuf,
        /// Optional exact placement constraints.
        #[arg(long)]
        placements: Option<PathBuf>,
        /// Write a neutral runtime report after execution.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Exact canonical Body construction source used for Host and Line truth.
        #[arg(long)]
        body: Option<PathBuf>,
        /// Do not attach standard-input cancellation; wait for the Play's own terminal outcome.
        #[arg(long)]
        await_terminal: bool,
    },
    /// Check, inspect, or build canonical Host construction truth.
    Host {
        #[command(subcommand)]
        command: HostCommand,
    },
    /// Check, inspect, or build canonical Body construction truth.
    Body {
        #[command(subcommand)]
        command: BodyCommand,
    },
    /// Operate finite self-hosted rendezvous infrastructure.
    RendezvousRelay {
        #[command(subcommand)]
        command: RendezvousRelayCommand,
    },
    /// Check a Form and render owned diagnostics without executing it.
    Check {
        form: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Inspect retained Conduit artifacts without executing work.
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },
    /// Run the protected local file-copy task.
    Copy {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum RendezvousRelayCommand {
    /// Create one private relay slot and two exact endpoint descriptors.
    Provision {
        /// Native socket address used by hosted endpoints to reach the relay.
        #[arg(long)]
        relay_address: String,
        /// Browser/native reachable WSS URL covered by the relay certificate.
        #[arg(long)]
        relay_url: String,
        /// Exact WebPKI/TLS server identity in the relay URL.
        #[arg(long)]
        server_identity: String,
        /// Lowercase or uppercase SHA-256 hex digest of the relay leaf certificate.
        #[arg(long)]
        certificate_sha256: String,
        #[arg(long)]
        first_host_id: String,
        #[arg(long)]
        first_boot_id: String,
        #[arg(long)]
        second_host_id: String,
        #[arg(long)]
        second_boot_id: String,
        /// New directory that will receive three private descriptor files.
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=3600))]
        expires_in_seconds: u64,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        maximum_attempts: u8,
        /// Explicitly authorize creation of new rendezvous secrets.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_provision: bool,
    },
    /// Serve one configured two-endpoint opaque relay slot over pinned WSS.
    Serve {
        /// Exact non-loopback socket explicitly exposed by this relay.
        #[arg(long)]
        bind: String,
        /// Browser/native reachable wss URL covered by the TLS certificate.
        #[arg(long)]
        public_url: String,
        /// PEM certificate chain for the relay's pinned outer identity.
        #[arg(long)]
        tls_cert: PathBuf,
        /// PEM private key for the relay's pinned outer identity.
        #[arg(long)]
        tls_key: PathBuf,
        /// Bounded private relay-slot configuration.
        #[arg(long)]
        slot: PathBuf,
        /// Stop if both endpoints do not attach within this many seconds.
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=3600))]
        accept_timeout_seconds: u64,
        /// Explicitly authorize listening beyond loopback.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_network: bool,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub(crate) enum PatchbayHost {
    Native,
    Browser,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HostCommand {
    /// Install, run, or inspect the durable local Host owner.
    Service {
        #[command(subcommand)]
        command: HostServiceCommand,
    },
    /// Obtain one target's reviewed release manifest from HTTPS or an offline mirror.
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
    /// Carry an exact Body-bound artifact without rebuilding it.
    Carry {
        #[command(subcommand)]
        command: CarrierCommand,
    },
    Check {
        source: PathBuf,
    },
    Show {
        source: PathBuf,
    },
    Build {
        source: PathBuf,
        #[arg(long, default_value = "target/host-build")]
        output: PathBuf,
    },
    /// Offer this already-running local Host to the browser Crèche.
    Rendezvous {
        /// Installed durable Host state owned by the running service.
        #[arg(long)]
        state_dir: PathBuf,
        /// Line carrier used to reach this running Host.
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
    /// Copy the exact artifact without starting or writing a Host.
    Download {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        destination: PathBuf,
    },
    /// Install and start an exact Body-bound native Host package locally.
    InstallNative {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        /// Durable local Host state retained across service restarts.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize installation and service start on this machine.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_install: bool,
    },
    /// Flash an exact Body-bound RP2040 UF2 to one confirmed BOOTSEL volume.
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
    /// Serve an exact Body-bound boot image over finite HTTP Boot.
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
    },
    /// Run the durable Host in the foreground for a platform service manager.
    Run {
        #[arg(long)]
        state_dir: PathBuf,
    },
    /// Print the retained Host identity and current runtime status.
    Status {
        #[arg(long)]
        state_dir: PathBuf,
        /// Emit bounded machine-readable Host, Boot, and release identity truth.
        #[arg(long)]
        json: bool,
    },
    /// Configure already-local Whisper, Ollama, and Piper providers for this durable Host.
    ConfigureVoice {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        whisper_executable: PathBuf,
        #[arg(long)]
        whisper_model: PathBuf,
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=32))]
        whisper_threads: u8,
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=120))]
        whisper_timeout_seconds: u64,
        #[arg(long)]
        ollama_model: String,
        #[arg(long)]
        admitted_memory_mib: u32,
        #[arg(long)]
        piper_executable: PathBuf,
        #[arg(long)]
        piper_model: PathBuf,
        #[arg(long)]
        piper_config: PathBuf,
        #[arg(long)]
        piper_library_path: Option<PathBuf>,
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=120))]
        piper_timeout_seconds: u64,
        /// Explicitly authorize local provider discovery, warmup, and durable configuration.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_local_voice: bool,
    },
    /// Make this durable Host retain one exact validated Body biography.
    OwnBody {
        /// Exported `conduit.body/biography-evidence@2` document.
        evidence: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BodyCommand {
    /// Inspect the Body retained by this installed Host.
    Status {
        /// Installed durable Host state that owns or has joined the Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Emit bounded machine-readable Body, Host, Boot, and biography identity truth.
        #[arg(long)]
        json: bool,
    },
    /// Issue one bounded invitation from the Body owned by this installed Host.
    Invite {
        /// Installed durable Host state that owns the Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Invitation lifetime; never exceeds the architectural maximum.
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=600))]
        ttl_seconds: u64,
    },
    /// Accept one bounded invitation on this installed Host and emit an admission request.
    Accept {
        /// Invitation JSON path, or `-` to read the exact document from standard input.
        invitation: PathBuf,
        /// Installed durable Host state that will join the invited Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize this Host to request membership in the invited Body.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_join: bool,
    },
    /// Admit one signed request into the Body owned by this installed Host.
    Admit {
        /// Admission-request JSON path, or `-` to read the exact document from standard input.
        request: PathBuf,
        /// Installed durable Host state that owns the Body.
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize adding the requested Host as a Body Part.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_admission: bool,
    },
    /// Retain the owner's exact admission receipt as this Host's durable membership.
    CompleteJoin {
        /// Owner-issued `conduit.body/spawn-admission-receipt@1` document.
        receipt: PathBuf,
        #[arg(long)]
        state_dir: PathBuf,
        /// Explicitly authorize retaining membership in the admitted Body.
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_membership: bool,
    },
    Check {
        source: PathBuf,
    },
    Show {
        source: PathBuf,
    },
    Build {
        source: PathBuf,
        #[arg(long, default_value = "target/body-build")]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum InspectCommand {
    /// Render a neutral runtime report.
    RuntimeReport { report: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn installed_help_exposes_the_body_growth_workflow_without_repository_commands() {
        let mut command = Cli::command();
        let mut rendered = Vec::new();
        command.write_long_help(&mut rendered).unwrap();
        let help = String::from_utf8(rendered).unwrap();

        for entrance in [
            "conduit body invite",
            "conduit body accept",
            "conduit body admit",
            "conduit body complete-join",
            "conduit host obtain",
            "conduit host carry",
            "conduit host service status",
        ] {
            assert!(help.contains(entrance), "missing {entrance} in:\n{help}");
        }
        assert!(help.contains("bounded JSON"));
        assert!(help.contains("without creating a Plan or Play"));
        assert!(help.contains("never implies carrier execution"));
        assert!(!help.contains("xtask"));
    }

    #[test]
    fn public_command_tree_parses() {
        assert!(matches!(
            Cli::try_parse_from([
                "conduit", "host", "carry", "availability", "download.json", "vm.json",
            ])
            .expect("carrier availability parses")
            .command,
            Command::Host {
                command: HostCommand::Carry {
                    command: CarrierCommand::Availability { descriptors }
                }
            } if descriptors.len() == 2
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "body",
                "complete-join",
                "receipt.json",
                "--state-dir",
                "installed",
                "--authorize-membership",
            ])
            .expect("joining-side membership completion parses")
            .command,
            Command::Body {
                command: BodyCommand::CompleteJoin {
                    authorize_membership: true,
                    ..
                }
            }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "body",
                "status",
                "--state-dir",
                "installed",
                "--json",
            ])
            .expect("current Body status parses")
            .command,
            Command::Body {
                command: BodyCommand::Status { state_dir, json: true }
            } if state_dir == std::path::Path::new("installed")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "carry",
                "download",
                "carrier.json",
                "artifact.json",
                "spore.iso",
                "download.iso",
            ])
            .expect("artifact-only carrier parses")
            .command,
            Command::Host {
                command: HostCommand::Carry {
                    command: CarrierCommand::Download { destination, .. }
                }
            } if destination == std::path::Path::new("download.iso")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "carry",
                "flash-uf2",
                "carrier.json",
                "artifact.json",
                "spore.uf2",
                "/media/RPI-RP2",
                "--confirm-volume",
                "/media/RPI-RP2",
                "--authorize-flash",
            ])
            .expect("consequential RP2040 UF2 carrier parses")
            .command,
            Command::Host {
                command: HostCommand::Carry {
                    command: CarrierCommand::FlashUf2 {
                        volume,
                        confirm_volume,
                        authorize_flash: true,
                        ..
                    }
                }
            } if volume == std::path::Path::new("/media/RPI-RP2")
                && confirm_volume == "/media/RPI-RP2"
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "carry",
                "launch-vm",
                "carrier.json",
                "artifact.json",
                "spore.iso",
                "--authorize-launch",
            ])
            .expect("consequential VM carrier parses")
            .command,
            Command::Host {
                command: HostCommand::Carry {
                    command: CarrierCommand::LaunchVm {
                        authorize_launch: true,
                        ..
                    }
                }
            }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "carry",
                "serve-http-boot",
                "carrier.json",
                "artifact.json",
                "spore.iso",
                "--bind",
                "0.0.0.0:8080",
                "--maximum-requests",
                "2",
                "--timeout-seconds",
                "30",
                "--authorize-serve",
            ])
            .expect("consequential HTTP Boot carrier parses")
            .command,
            Command::Host {
                command: HostCommand::Carry {
                    command: CarrierCommand::ServeHttpBoot {
                        bind,
                        maximum_requests: 2,
                        timeout_seconds: 30,
                        authorize_serve: true,
                        ..
                    }
                }
            } if bind == "0.0.0.0:8080"
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "home"])
                .expect("Home entrance parses")
                .command,
            Command::Home
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "obtain",
                "conduitos/x86_64/pc",
                "--catalog",
                "release/catalog.json",
                "--catalog-id",
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "--mirror",
                "release",
                "--cache",
                "installed-cache",
            ])
            .expect("installed target obtain entrance parses")
            .command,
            Command::Host {
                command: HostCommand::Obtain { target, .. }
            } if target == "conduitos/x86_64/pc"
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "body",
                "accept",
                "-",
                "--state-dir",
                "installed-host",
                "--authorize-join",
            ])
            .expect("scriptable Body invitation acceptance parses")
            .command,
            Command::Body {
                command: BodyCommand::Accept {
                    invitation,
                    state_dir,
                    authorize_join: true,
                }
            } if invitation == std::path::Path::new("-")
                && state_dir == std::path::Path::new("installed-host")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "body",
                "admit",
                "-",
                "--state-dir",
                "body-owner",
                "--authorize-admission",
            ])
            .expect("scriptable Body admission parses")
            .command,
            Command::Body {
                command: BodyCommand::Admit {
                    request,
                    state_dir,
                    authorize_admission: true,
                }
            } if request == std::path::Path::new("-")
                && state_dir == std::path::Path::new("body-owner")
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "creche"])
                .expect("Crèche entrance parses")
                .command,
            Command::Creche
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit", "host", "service", "own-body", "body.json", "--state-dir", "installed-host",
            ])
            .expect("durable Body ownership entrance parses")
            .command,
            Command::Host {
                command: HostCommand::Service {
                    command: HostServiceCommand::OwnBody { evidence, state_dir }
                }
            } if evidence == std::path::Path::new("body.json")
                && state_dir == std::path::Path::new("installed-host")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "service",
                "status",
                "--state-dir",
                "installed-host",
            ])
            .expect("durable Host service entrance parses")
            .command,
            Command::Host {
                command: HostCommand::Service {
                    command: HostServiceCommand::Status { state_dir, json: false }
                }
            } if state_dir == std::path::Path::new("installed-host")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit", "host", "rendezvous", "--state-dir", "installed-host", "--carrier",
                "secure-websocket", "--bind", "192.0.2.10:7443", "--public-url",
                "wss://host.example:7443/conduit", "--tls-cert", "host-cert.pem", "--tls-key",
                "host-key.pem", "--authorize-network",
            ])
            .expect("explicit secure LAN rendezvous entrance parses")
            .command,
            Command::Host {
                command: HostCommand::Rendezvous {
                    carrier: RendezvousCarrier::SecureWebsocket,
                    authorize_network: true,
                    bind: Some(bind),
                    public_url: Some(public_url),
                    tls_cert: Some(cert),
                    tls_key: Some(key),
                    ..
                }
            } if bind == "192.0.2.10:7443"
                && public_url == "wss://host.example:7443/conduit"
                && cert == std::path::Path::new("host-cert.pem")
                && key == std::path::Path::new("host-key.pem")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "service",
                "status",
                "--state-dir",
                "installed-host",
                "--json",
            ])
            .expect("scriptable durable Host status parses")
            .command,
            Command::Host {
                command: HostCommand::Service {
                    command: HostServiceCommand::Status { json: true, .. }
                }
            }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "rendezvous",
                "--state-dir",
                "installed-host",
                "--timeout-seconds", "30"
            ])
                .expect("Host rendezvous entrance parses")
                .command,
            Command::Host {
                command: HostCommand::Rendezvous {
                    state_dir,
                    carrier: RendezvousCarrier::Websocket,
                    timeout_seconds: 30,
                    bind: None,
                    public_url: None,
                    tls_cert: None,
                    tls_key: None,
                    relay_descriptor: None,
                    authorize_network: false,
                }
            } if state_dir == std::path::Path::new("installed-host")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "host",
                "rendezvous",
                "--state-dir",
                "installed-host",
                "--carrier",
                "relay",
                "--relay-descriptor",
                "relay-endpoint.json",
            ])
            .expect("outbound protected relay Host entrance parses")
            .command,
            Command::Host {
                command: HostCommand::Rendezvous {
                    carrier: RendezvousCarrier::Relay,
                    relay_descriptor: Some(descriptor),
                    ..
                }
            } if descriptor == std::path::Path::new("relay-endpoint.json")
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "rendezvous-relay",
                "provision",
                "--relay-address",
                "192.0.2.10:7443",
                "--relay-url",
                "wss://relay.example:7443/conduit",
                "--server-identity",
                "relay.example",
                "--certificate-sha256",
                "1111111111111111111111111111111111111111111111111111111111111111",
                "--first-host-id",
                "host/first",
                "--first-boot-id",
                "boot/first",
                "--second-host-id",
                "host/second",
                "--second-boot-id",
                "boot/second",
                "--output",
                "private-relay",
                "--authorize-provision",
            ])
            .expect("private relay provisioning entrance parses")
            .command,
            Command::RendezvousRelay {
                command: RendezvousRelayCommand::Provision {
                    relay_address,
                    maximum_attempts: 1,
                    authorize_provision: true,
                    ..
                }
            } if relay_address == "192.0.2.10:7443"
        ));
        assert!(Cli::try_parse_from([
            "conduit",
            "rendezvous-relay",
            "provision",
            "--relay-address",
            "192.0.2.10:7443",
            "--relay-url",
            "wss://relay.example:7443/conduit",
            "--server-identity",
            "relay.example",
            "--certificate-sha256",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "--first-host-id",
            "host/first",
            "--first-boot-id",
            "boot/first",
            "--second-host-id",
            "host/second",
            "--second-boot-id",
            "boot/second",
            "--output",
            "private-relay",
        ])
        .is_err());
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "rendezvous-relay",
                "serve",
                "--bind",
                "192.0.2.10:7443",
                "--public-url",
                "wss://relay.example:7443/conduit",
                "--tls-cert",
                "relay-cert.pem",
                "--tls-key",
                "relay-key.pem",
                "--slot",
                "private-slot.json",
                "--accept-timeout-seconds",
                "30",
                "--authorize-network",
            ])
            .expect("bounded user-operated relay entrance parses")
            .command,
            Command::RendezvousRelay {
                command: RendezvousRelayCommand::Serve {
                    bind,
                    public_url,
                    slot,
                    accept_timeout_seconds: 30,
                    authorize_network: true,
                    ..
                }
            } if bind == "192.0.2.10:7443"
                && public_url == "wss://relay.example:7443/conduit"
                && slot == std::path::Path::new("private-slot.json")
        ));
        assert!(Cli::try_parse_from([
            "conduit",
            "rendezvous-relay",
            "serve",
            "--bind",
            "192.0.2.10:7443",
            "--public-url",
            "wss://relay.example:7443/conduit",
            "--tls-cert",
            "relay-cert.pem",
            "--tls-key",
            "relay-key.pem",
            "--slot",
            "private-slot.json",
        ])
        .is_err());
        assert!(matches!(
            Cli::try_parse_from(["conduit", "patchbay", "--on", "browser"])
                .expect("Patchbay browser entrance parses")
                .command,
            Command::Patchbay {
                on: PatchbayHost::Browser,
                body_evidence: None,
                body_invitation: None,
                reviewed_form,
            } if reviewed_form.is_empty()
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "patchbay",
                "--on",
                "browser",
                "--body-evidence",
                "roseau.json"
            ])
            .expect("exported Body evidence entrance parses")
            .command,
            Command::Patchbay {
                on: PatchbayHost::Browser,
                body_evidence: Some(path),
                body_invitation: None,
                reviewed_form,
            } if path == std::path::Path::new("roseau.json") && reviewed_form.is_empty()
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit", "patchbay", "--on", "browser", "--body-evidence", "roseau.json",
                "--reviewed-form", "Greet", "forms/greet/greet.conduit", "--reviewed-form",
                "Count", "forms/count/count.conduit",
            ])
            .expect("reviewed adult Form inventory parses")
            .command,
            Command::Patchbay { reviewed_form, .. } if reviewed_form.len() == 4
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "patchbay",
                "--on",
                "browser",
                "--body-evidence",
                "roseau.json",
                "--body-invitation",
                "ws://127.0.0.1:4173/body",
            ])
            .expect("explicit local Body invitation parses")
            .command,
            Command::Patchbay { body_invitation: Some(url), .. }
                if url == "ws://127.0.0.1:4173/body"
        ));
        assert!(Cli::try_parse_from([
            "conduit",
            "patchbay",
            "--on",
            "browser",
            "--body-invitation",
            "ws://127.0.0.1:4173/body",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "conduit",
            "patchbay",
            "--on",
            "browser",
            "--reviewed-form",
            "Greet",
            "forms/greet/greet.conduit",
        ])
        .is_err());
        assert!(matches!(
            Cli::try_parse_from(["conduit", "run", "hello.conduit"])
                .expect("run command parses")
                .command,
            Command::Run { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "run", "finite.conduit", "--await-terminal"])
                .expect("explicit terminal-wait run parses")
                .command,
            Command::Run {
                await_terminal: true,
                ..
            }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "run",
                "signal.conduit",
                "--body",
                "current.body.conduit"
            ])
            .expect("Body-backed product run parses")
            .command,
            Command::Run { body: Some(_), .. }
        ));
        assert!(Cli::try_parse_from([
            "conduit",
            "run",
            "signal.conduit",
            "--execution-fixture",
            "two-std-line"
        ])
        .is_err());
        assert!(matches!(
            Cli::try_parse_from(["conduit", "check", "hello.conduit", "--json"])
                .expect("check command parses")
                .command,
            Command::Check { json: true, .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "inspect", "runtime-report", "run.json"])
                .expect("inspect command parses")
                .command,
            Command::Inspect { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "host", "build", "linux.host.conduit"])
                .expect("Host build entrance parses")
                .command,
            Command::Host {
                command: HostCommand::Build { .. }
            }
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "conduit",
                "body",
                "invite",
                "--state-dir",
                "installed-host",
                "--ttl-seconds",
                "30",
            ])
            .expect("scriptable Body invitation parses")
            .command,
            Command::Body {
                command: BodyCommand::Invite {
                    state_dir,
                    ttl_seconds: 30,
                }
            } if state_dir == std::path::Path::new("installed-host")
        ));
        assert!(matches!(
            Cli::try_parse_from(["conduit", "body", "show", "current.body.conduit"])
                .expect("Body show entrance parses")
                .command,
            Command::Body {
                command: BodyCommand::Show { .. }
            }
        ));
    }
}
