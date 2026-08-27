mod impact;

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct CiArgs {
    #[command(subcommand)]
    command: CiCommand,
}

#[derive(Subcommand, Debug)]
enum CiCommand {
    /// Plan heavyweight CI obligations for one exact Git diff.
    Plan {
        /// Exact base commit SHA.
        base: String,
        /// Exact head commit SHA.
        head: String,
        /// Write the complete machine-readable plan to this path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Write a Markdown job-summary table to this path.
        #[arg(long)]
        summary_out: Option<PathBuf>,
    },
}

pub fn run(args: CiArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        CiCommand::Plan {
            base,
            head,
            json_out,
            summary_out,
        } => impact::run(&base, &head, json_out.as_deref(), summary_out.as_deref()),
    }
}
