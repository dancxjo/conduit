mod standalone_locks;

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct CiArgs {
    #[command(subcommand)]
    command: CiCommand,
}

#[derive(Subcommand, Debug)]
enum CiCommand {
    /// Fail fast when a separately rooted make lock is stale.
    StandaloneLocks,
}

pub fn run(args: CiArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        CiCommand::StandaloneLocks => standalone_locks::run(),
    }
}
