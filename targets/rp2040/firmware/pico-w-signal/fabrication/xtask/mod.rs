mod appliance_identity;
mod body_admission;
mod bootsel;
mod capstone_serial;
mod create_battery_rx;
mod create_full_stage;
mod create_hello;
mod create_lights_stage;
mod create_listen;
mod create_motion;
mod create_power;
mod create_presentation;
mod doctor;
mod firmware;
#[cfg(test)]
mod firmware_tests;
mod flash;
mod indicator_build;
mod prove_appliance;
mod prove_appliance_hil;
mod prove_usb;
#[cfg(unix)]
mod prove_websocket;
mod prove_wifi;
mod r1_control_session;
#[cfg(unix)]
mod r1_full;
mod r1_lifecycle;
mod r1_live_control;
mod r1_signal;
mod r1_signal_transcript;
mod serial;
#[cfg(unix)]
mod session_completion;
#[cfg(unix)]
mod session_failure;
mod transcript;
#[cfg(unix)]
mod usb_continuity;
mod wifi_secrets;

use crate::cli::GlobalOpts;
use crate::output::{OutputMode, RepositoryOutput};
use clap::{Args, Subcommand};
use serde::Serialize;

pub type PicoResult<T> = Result<T, Box<dyn std::error::Error>>;

pub use bootsel::run_bootsel;
pub use doctor::run_doctor;
pub use firmware::read_identity_manifest;
pub use firmware::run_build;
pub use flash::run_flash;
pub use prove_appliance::run_prove_pico_appliance;
pub use prove_appliance_hil::run_prove_pico_appliance_hil;
pub use prove_usb::run_prove_std_pico_usb;
pub use prove_wifi::run_prove_pico_wifi_bootstrap;
pub use prove_wifi::WifiProofMode;
pub use serial::run_verify;
pub use transcript::{verify_bluetooth_loss_transcript, verify_bluetooth_transcript};

/// Arguments shared across all Pico subcommands and the top-level `pico-local` alias.
#[derive(Args, Clone, Debug, Default)]
pub struct PicoArgs {
    #[command(subcommand)]
    pub subcommand: Option<PicoSubcommand>,

    /// Set from xtask's global `--dry-run` option after parsing.
    #[arg(skip)]
    pub dry_run: bool,

    /// Build firmware only; do not flash or verify.
    #[arg(long, global = true)]
    pub build_only: bool,

    /// Explicit BOOTSEL mount point (overrides auto-discovery and PICO_W_MOUNT).
    #[arg(long, global = true)]
    pub mount: Option<String>,

    /// Explicit USB CDC serial port (overrides auto-discovery and PICO_W_PORT).
    #[arg(long, global = true)]
    pub port: Option<String>,

    /// Explicit CDC 0 link port used for the BOOTSEL reboot request.
    #[arg(long, global = true)]
    pub link_port: Option<String>,

    /// Verify firmware build but skip flashing and live hardware check.
    #[arg(long, global = true)]
    pub verify: bool,

    /// Build or flash the explicit std-to-Pico USB remote image.
    #[arg(long, global = true)]
    pub usb_remote: bool,

    /// Build or flash the final exact three-host remote sink image.
    #[arg(long, global = true)]
    pub triple_remote: bool,

    /// Build or flash the R1 USB-authorized Wi-Fi bootstrap image.
    #[arg(long, global = true)]
    pub wifi_bootstrap: bool,

    /// Build or flash the R1 three-peer control image over WebSocket and USB CDC.
    #[arg(long, global = true)]
    pub r1_control: bool,

    /// Build or flash the finite open AP/DHCP/DNS/HTTP Hello appliance image.
    #[arg(long, global = true)]
    pub appliance_hello: bool,

    /// Build or flash the fixture-only second-Pico appliance client probe.
    #[arg(long, global = true)]
    pub appliance_hil_client: bool,

    /// Build or flash the finite Pico W BLE GATT Line image.
    #[arg(long, global = true)]
    pub bluetooth_line: bool,

    /// Build or flash the exact three-participant distributed Lenia worker image.
    #[arg(long, global = true)]
    pub distributed_lenia: bool,

    /// Build the bounded breadboard USB-MIDI fixture image (build only; never flash).
    #[arg(long, global = true)]
    pub usb_midi_fixture: bool,

    /// Build, flash, or verify the Pete capstone physical Play image.
    #[arg(long = "pete-capstone", global = true)]
    pub pete_capstone: bool,

    /// Build or flash the bounded, acquired indicator peripheral image.
    #[arg(long, global = true)]
    pub indicator_resource: bool,

    /// Re-download and re-verify the vendored CYW43 radio assets from the pinned commit.
    #[arg(long, global = true)]
    pub refresh_radio_assets: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum PicoSubcommand {
    /// Check prerequisites and vendored assets.
    Doctor,
    /// Build firmware and produce UF2.
    Build,
    /// Flash UF2 to a Pico W in BOOTSEL mode.
    Flash,
    /// Verify USB receipts from a running Pico W.
    Verify,
    /// Ask the exact running firmware to reboot into BOOTSEL over CDC 0.
    Bootsel,
    /// Acquire Create Full, play one hello, and restore Safe without wheel authority.
    HelloCreate,
    /// Send only START/FULL, hold briefly, then restore Safe without observing RX.
    FullCreate {
        /// Confirm that the robot is stopped and physically clear for the attended test.
        #[arg(long)]
        wheels_clear: bool,
    },
    /// Run only the Netherwick Create-light pattern, then restore Safe.
    LightsCreate {
        /// Confirm that the robot is stopped and physically clear for the attended test.
        #[arg(long)]
        wheels_clear: bool,
    },
    /// Observe Create RX for one second with OE high and UART TX exactly zero.
    ListenCreate,
    /// Run one attended 250 ms wheels-off-floor semantic motion proof.
    DriveCreate {
        /// Confirm that every drive wheel is securely off the floor.
        #[arg(long)]
        wheels_off_floor: bool,
    },
    /// Emit one attended Create power-toggle pulse after physical off-state confirmation.
    WakeCreate {
        /// Confirm that the Create is physically observed off before toggling power.
        #[arg(long)]
        confirmed_off: bool,
    },
    /// Play one bounded original riff with the Netherwick supervision lights.
    PresentCreate {
        /// Confirm that the robot is stopped and physically clear for the attended test.
        #[arg(long)]
        wheels_clear: bool,
    },
    /// Read one bounded Create OI packet-0 battery sample and restore Safe.
    ReadCreateBattery {
        /// Confirm that the robot is stopped, attended, and unable to propel itself.
        #[arg(long)]
        wheels_clear: bool,
    },
    /// Prove explicit Body admission against an already-provisioned Pico.
    ProveBodyAdmission,
    /// Full local workflow: doctor + build + flash + verify.
    Local,
}

impl PicoSubcommand {
    const fn command_name(&self) -> &'static str {
        match self {
            Self::Doctor => "pico doctor",
            Self::Build => "pico build",
            Self::Flash => "pico flash",
            Self::Verify => "pico verify",
            Self::Bootsel => "pico bootsel",
            Self::HelloCreate => "pico hello-create",
            Self::FullCreate { .. } => "pico full-create",
            Self::LightsCreate { .. } => "pico lights-create",
            Self::ListenCreate => "pico listen-create",
            Self::DriveCreate { .. } => "pico drive-create",
            Self::WakeCreate { .. } => "pico wake-create",
            Self::PresentCreate { .. } => "pico present-create",
            Self::ReadCreateBattery { .. } => "pico read-create-battery",
            Self::ProveBodyAdmission => "pico prove-body-admission",
            Self::Local => "pico local",
        }
    }

    const fn attended(&self) -> bool {
        matches!(
            self,
            Self::HelloCreate
                | Self::FullCreate { .. }
                | Self::LightsCreate { .. }
                | Self::ListenCreate
                | Self::DriveCreate { .. }
                | Self::WakeCreate { .. }
                | Self::PresentCreate { .. }
                | Self::ReadCreateBattery { .. }
                | Self::ProveBodyAdmission
        )
    }
}

#[derive(Serialize)]
struct PicoDryRunReport {
    schema: &'static str,
    command: &'static str,
    disposition: &'static str,
    effects_performed: bool,
    attended: bool,
}

pub fn prepare_output(opts: &GlobalOpts, args: &PicoArgs) -> PicoResult<bool> {
    let output = RepositoryOutput::from_opts(opts);
    if output.mode() == OutputMode::Human {
        return Ok(true);
    }
    let subcommand = args.subcommand.as_ref().unwrap_or(&PicoSubcommand::Local);
    if output.dry_run() {
        output.emit_json(&PicoDryRunReport {
            schema: "conduit.xtask/pico-dry-run@1",
            command: subcommand.command_name(),
            disposition: "planned-not-dispatched",
            effects_performed: false,
            attended: subcommand.attended(),
        })?;
        return Ok(false);
    }
    let reason = if subcommand.attended() {
        "attended interaction requires visible prompts and physical distinctions"
    } else {
        "the command has not declared bounded structured or quiet live output"
    };
    output.refusal(
        subcommand.command_name(),
        match output.mode() {
            OutputMode::Json => "json",
            OutputMode::Quiet => "quiet",
            OutputMode::Human => unreachable!(),
        },
        reason,
    )?;
    Ok(true)
}

pub fn apply_environment_defaults(args: &mut PicoArgs) {
    if args.mount.is_none() {
        args.mount = std::env::var("PICO_W_MOUNT").ok();
    }
    if args.port.is_none() {
        args.port = std::env::var("PICO_W_PORT").ok();
    }
    if args.link_port.is_none() {
        args.link_port = std::env::var("PICO_W_LINK_PORT").ok();
    }
}

pub fn run(mut args: PicoArgs) -> PicoResult<()> {
    apply_environment_defaults(&mut args);
    if usize::from(args.usb_remote)
        + usize::from(args.triple_remote)
        + usize::from(args.wifi_bootstrap)
        + usize::from(args.r1_control)
        + usize::from(args.appliance_hello)
        + usize::from(args.appliance_hil_client)
        + usize::from(args.bluetooth_line)
        + usize::from(args.distributed_lenia)
        + usize::from(args.usb_midi_fixture)
        + usize::from(args.pete_capstone)
        + usize::from(args.indicator_resource)
        > 1
    {
        return Err("select only one remote Pico firmware mode".into());
    }
    if args.refresh_radio_assets {
        firmware::refresh_radio_assets(args.dry_run)?;
        return Ok(());
    }
    match &args.subcommand {
        None | Some(PicoSubcommand::Local) => run_local(args),
        Some(PicoSubcommand::Doctor) => run_doctor(args.dry_run),
        Some(PicoSubcommand::Build) => run_build(&args),
        Some(PicoSubcommand::Flash) => run_flash(&args),
        Some(PicoSubcommand::Verify) => run_verify(&args),
        Some(PicoSubcommand::Bootsel) => run_bootsel(&args),
        Some(PicoSubcommand::HelloCreate) => create_hello::run(&args),
        Some(PicoSubcommand::FullCreate { wheels_clear }) => {
            create_full_stage::run(&args, *wheels_clear)
        }
        Some(PicoSubcommand::LightsCreate { wheels_clear }) => {
            create_lights_stage::run(&args, *wheels_clear)
        }
        Some(PicoSubcommand::ListenCreate) => create_listen::run(&args),
        Some(PicoSubcommand::DriveCreate { wheels_off_floor }) => {
            create_motion::run(&args, *wheels_off_floor)
        }
        Some(PicoSubcommand::WakeCreate { confirmed_off }) => {
            create_power::run(&args, *confirmed_off)
        }
        Some(PicoSubcommand::PresentCreate { wheels_clear }) => {
            create_presentation::run(&args, *wheels_clear)
        }
        Some(PicoSubcommand::ReadCreateBattery { wheels_clear }) => {
            create_battery_rx::run(&args, *wheels_clear)
        }
        Some(PicoSubcommand::ProveBodyAdmission) => body_admission::run(&args),
    }
}

pub fn run_local(mut args: PicoArgs) -> PicoResult<()> {
    apply_environment_defaults(&mut args);
    if args.usb_remote
        || args.triple_remote
        || args.wifi_bootstrap
        || args.r1_control
        || args.appliance_hello
        || args.bluetooth_line
        || args.distributed_lenia
        || args.usb_midi_fixture
        || args.pete_capstone
        || args.indicator_resource
    {
        return Err("the complete `pico local` workflow requires the pico-local image; use `pico build --usb-remote`, `pico flash --usb-remote`, then `prove std-pico-usb` for the remote proof".into());
    }
    run_doctor(args.dry_run)?;
    run_build(&args)?;
    if args.build_only {
        return Ok(());
    }
    run_flash(&args)?;
    run_verify(&args)
}
