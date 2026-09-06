//! Repository-only orchestration entry point for Conduit development and proof tooling.
//!
//! Product-facing form execution remains in the Conduit CLI; this binary owns only
//! checked-out repository workflows and local hardware tooling.

mod cli;
mod command_registry;
mod commands;
mod evidence;
mod obligation;
mod output;
mod process;
mod proof;
mod suites;
mod workspace;

use clap::Parser;
use cli::{AudioCommand, Cli, Command, DemoCommand, GlobalOpts, MidiCommand};

fn main() {
    let cli = Cli::parse_from(command_registry::normalize_compatibility_aliases(
        std::env::args_os().collect(),
    ));
    let opts = cli.global;
    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Command::Avr(args) => commands::avr::run(args, &opts),
        Command::Browser => commands::browser::run(&opts),
        Command::Body(args) => commands::body::run(args, &opts),
        Command::BodyCoordination(args) => commands::body_coordination::run(args, &opts),
        Command::Catalog(args) => commands::catalog::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Check(args) => commands::check::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Ci(args) => commands::ci::run(args),
        Command::Prove(args) => commands::prove::run(*args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Proofs(args) => commands::proofs::run(args, opts.json),
        Command::Evidence(args) => commands::evidence::run(args),
        Command::Forms(args) => commands::forms::run(args, &opts)
            .map_err(|error| Box::new(std::io::Error::other(error)) as Box<dyn std::error::Error>),
        Command::Doctor(args) => commands::doctor::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Esp32Firmware(args) => commands::esp32_firmware::run(args, &opts),
        Command::Pico(mut args) => run_pico(&opts, &mut args, false),
        Command::Host(args) => commands::host::run(args, &opts),
        Command::PicoLocal(mut args) => run_pico(&opts, &mut args, true),
        Command::Conduitos(args) => commands::conduitos::run(args, &opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Command::Audio(args) => match args.command {
            AudioCommand::List => commands::audio::list(&opts),
            AudioCommand::PlaybackProof {
                card_id,
                device,
                authorize_output,
            } => commands::audio::prove(&opts, &card_id, device, authorize_output),
        },
        Command::Midi(args) => match args.command {
            MidiCommand::List => commands::midi::list(&opts),
        },
        Command::Pete(args) => commands::pete_std_observe::run(args, &opts),
        Command::Demo(args) => match args.command {
            DemoCommand::Tour => commands::demo::run_tour(&opts),
            DemoCommand::Std => commands::demo::run_std(&opts),
            DemoCommand::Triple => commands::demo::run_triple(&opts),
            DemoCommand::Patchbay(args) => commands::demo::run_patchbay(&args, &opts),
            DemoCommand::BodyMembership => commands::demo::run_body_membership(&opts),
            DemoCommand::Environment => commands::demo::run_environment(&opts),
            DemoCommand::Prewake => commands::demo::run_prewake(&opts),
            DemoCommand::TextLab => commands::demo::run_text_lab(&opts),
            DemoCommand::Toggle => commands::toggle::run(),
            DemoCommand::LightSwitch(args) => commands::light_switch::run(args),
            DemoCommand::ButtonIndicator(args) => commands::button_indicator::run(args, &opts),
            DemoCommand::Site => commands::toggle::run_site(),
            DemoCommand::Tongues => commands::tongues::run(&opts),
            DemoCommand::TonguesResearch => commands::tongues::run_research(&opts),
            DemoCommand::TonguesAnalysis => commands::tongues::run_analysis(&opts),
        },
        Command::UnifontSubset(args) => commands::unifont_subset::run(args),
        Command::PaletteIcons(args) => commands::palette_icons::run(args),
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
    if !commands::pico::prepare_output(opts, args)? {
        return Ok(());
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
