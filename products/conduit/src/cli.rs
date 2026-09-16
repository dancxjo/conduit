use clap::{Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
use std::path::PathBuf;

/// Product command-line entrance for installed Conduit workflows.
#[derive(Debug, Parser)]
#[command(
    name = "conduit",
    about = "Run Forms and grow a living Conduit Body",
    long_about = "Run Forms and grow a living Conduit Body.\n\nUse `conduit body invite` to issue bounded joining authority, `conduit body accept` on an installed machine to prepare its signed admission request, and `conduit body admit` on the owning machine to commit membership and current presence. Use `conduit host obtain` to resolve a reviewed target release. Body binding remains separate from `conduit host carry`, which downloads, launches, writes, or flashes an exact artifact through an explicit carrier. Inspect durable identity and current runtime truth with `conduit host service status`.",
    after_help = "BODY GROWTH\n  1. conduit body invite --state-dir <OWNER_STATE> > invitation.json\n  2. conduit body accept invitation.json --state-dir <JOINING_STATE> --authorize-join > request.json\n  3. conduit body admit request.json --state-dir <OWNER_STATE> --authorize-admission > receipt.json\n  4. conduit host obtain <TARGET> --catalog <CATALOG> --catalog-id <ID> --mirror <MIRROR> --cache <CACHE>\n  5. conduit host carry <CARRIER> --help\n\nInvitation, request, and receipt documents are bounded JSON suitable for standard input/output. Admission reports membership and current offers without creating a Plan or Play. Artifact preparation never implies carrier execution, boot, admission, Plan, or Play."
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
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CarrierCommand {
    /// Copy the exact artifact without starting or writing a Host.
    Download {
        descriptor: PathBuf,
        artifact: PathBuf,
        source: PathBuf,
        destination: PathBuf,
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
    /// Newline-framed serial stream on standard input and output.
    Serial,
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
                    timeout_seconds: 30
                }
            } if state_dir == std::path::Path::new("installed-host")
        ));
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
