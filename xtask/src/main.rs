//! Repository-only orchestration entry point for Conduit development and proof tooling.
//!
//! Product-facing form execution remains in the Conduit CLI; this binary owns only
//! checked-out repository workflows and local hardware tooling.

mod cli;
mod commands;
mod obligation;
mod process;
mod proof;
mod suites;
mod workspace;

use clap::Parser;
use cli::{Cli, Command, DemoCommand, GlobalOpts};

fn main() {
    let cli = Cli::parse();
    let opts = cli.global;
    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Command::Check(args) => commands::check::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Prove(args) => commands::prove::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Proofs(args) => commands::proofs::run(args, opts.json),
        Command::Doctor(args) => commands::doctor::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Pico(mut args) => run_pico(&opts, &mut args, false),
        Command::PicoLocal(mut args) => run_pico(&opts, &mut args, true),
        Command::Conduitos(args) => commands::conduitos::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Demo(args) => match args.command {
            DemoCommand::Std => commands::demo::run_std(&opts),
            DemoCommand::Triple => commands::demo::run_triple(&opts),
            DemoCommand::Patchbay => commands::demo::run_patchbay(&opts),
            DemoCommand::Browser => commands::toggle::run(),
            DemoCommand::Toggle => commands::toggle::run(),
            DemoCommand::Site => commands::toggle::run_site(),
        },
    };

    if let Err(error) = result {
        eprintln!("xtask error: {error}");
        std::process::exit(1);
    }
}

fn run_pico(
    opts: &GlobalOpts,
    args: &mut commands::pico::PicoArgs,
    local_alias: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        return Err("--json is not yet supported by Pico hardware commands".into());
    }
    if opts.quiet {
        return Err("--quiet is not yet supported by Pico hardware commands".into());
    }
    args.dry_run = opts.dry_run;
    let owned = args.clone();
    if local_alias {
        commands::pico::run_local(owned)?;
    } else {
        commands::pico::run(owned)?;
    }
    Ok(())
}
