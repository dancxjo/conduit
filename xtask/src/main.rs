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
        Command::Check(args) => commands::check::run(args, &opts),
        Command::Demo(args) => commands::demo::run(args, &opts),
        Command::Prove(args) => commands::prove::run(args, &opts),
        Command::Doctor(args) => commands::doctor::run(args, &opts),
    };
    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}
