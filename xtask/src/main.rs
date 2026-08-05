//! Repository-only orchestration entry point for Conduit development and proof tooling.
//!
//! Product-facing form execution remains in the Conduit CLI; this binary owns only
//! checked-out repository workflows and read-only environment inspection.

mod cli;
mod commands;
mod process;
mod suites;
mod workspace;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let opts = cli.global;
    let result = match cli.command {
        Command::Doctor(args) => commands::doctor::run(args, &opts),
    };
    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
