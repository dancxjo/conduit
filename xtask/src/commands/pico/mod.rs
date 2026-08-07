mod doctor;
mod firmware;
mod flash;
mod prove_usb;
mod serial;

use clap::{Args, Subcommand};

pub type PicoResult<T> = Result<T, Box<dyn std::error::Error>>;

pub use doctor::run_doctor;
pub use firmware::run_build;
pub use flash::run_flash;
pub use prove_usb::run_prove_std_pico_usb;
pub use serial::run_verify;

/// Arguments shared across all Pico subcommands and the top-level `pico-local` alias.
#[derive(Args, Clone, Debug, Default)]
pub struct PicoArgs {
    #[command(subcommand)]
    pub subcommand: Option<PicoSubcommand>,

    /// Set from xtask's global `--dry-run` option after parsing.
    #[arg(skip)]
    pub dry_run: bool,

    /// Build firmware only; do not flash or verify.
    #[arg(long)]
    pub build_only: bool,

    /// Explicit BOOTSEL mount point (overrides auto-discovery and PICO_W_MOUNT).
    #[arg(long)]
    pub mount: Option<String>,

    /// Explicit USB CDC serial port (overrides auto-discovery and PICO_W_PORT).
    #[arg(long)]
    pub port: Option<String>,

    /// Verify firmware build but skip flashing and live hardware check.
    #[arg(long)]
    pub verify: bool,

    /// Re-download and re-verify the vendored CYW43 radio assets from the pinned commit.
    #[arg(long)]
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
    /// Full local workflow: doctor + build + flash + verify.
    Local,
}

pub fn apply_environment_defaults(args: &mut PicoArgs) {
    if args.mount.is_none() {
        args.mount = std::env::var("PICO_W_MOUNT").ok();
    }
    if args.port.is_none() {
        args.port = std::env::var("PICO_W_PORT").ok();
    }
}

pub fn run(mut args: PicoArgs) -> PicoResult<()> {
    apply_environment_defaults(&mut args);
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
    }
}

pub fn run_local(mut args: PicoArgs) -> PicoResult<()> {
    apply_environment_defaults(&mut args);
    run_doctor(args.dry_run)?;
    run_build(&args)?;
    if args.build_only {
        return Ok(());
    }
    run_flash(&args)?;
    run_verify(&args)
}
