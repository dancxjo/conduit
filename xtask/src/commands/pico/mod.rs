mod doctor;
mod firmware;
mod flash;
mod serial;

use anyhow::Result;
use clap::{Args, Subcommand};

pub use doctor::run_doctor;
pub use firmware::run_build;
pub use flash::run_flash;
pub use serial::run_verify;

/// Arguments shared across all pico sub-commands and the top-level `pico-local` alias.
#[derive(Args, Clone, Debug, Default)]
pub struct PicoArgs {
    #[command(subcommand)]
    pub subcommand: Option<PicoSubcommand>,

    /// Print every planned action without executing anything.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Build firmware only; do not flash or verify.
    #[arg(long, global = true)]
    pub build_only: bool,

    /// Explicit BOOTSEL mount point (overrides auto-discovery and PICO_W_MOUNT).
    #[arg(long, global = true, env = "PICO_W_MOUNT")]
    pub mount: Option<String>,

    /// Explicit USB CDC serial port (overrides auto-discovery and PICO_W_PORT).
    #[arg(long, global = true, env = "PICO_W_PORT")]
    pub port: Option<String>,

    /// Verify firmware build but skip flashing and live hardware check.
    #[arg(long, global = true)]
    pub verify: bool,

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
    /// Full local workflow: doctor + build + flash + verify.
    Local,
}

/// Entry point for `cargo xtask pico <subcommand>`.
pub fn run(args: PicoArgs) -> Result<()> {
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

/// Entry point for `cargo xtask pico-local` (full workflow).
pub fn run_local(args: PicoArgs) -> Result<()> {
    run_doctor(args.dry_run)?;
    run_build(&args)?;
    if args.build_only {
        return Ok(());
    }
    run_flash(&args)?;
    run_verify(&args)
}
