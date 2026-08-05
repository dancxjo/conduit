mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::pico::PicoArgs;

#[derive(Parser)]
#[command(name = "xtask", about = "Conduit development task runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pico W local LED proof workflow
    Pico(PicoArgs),
    /// Alias for `pico local` (one-command build+flash+verify)
    PicoLocal(PicoArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Pico(args) => commands::pico::run(args),
        Commands::PicoLocal(args) => commands::pico::run_local(args),
    }
}
