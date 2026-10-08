//! Repository-only orchestration entry point for Conduit development and proof tooling.
//!
//! Product-facing plot execution remains in the Conduit CLI; this binary owns only
//! checked-out repository workflows and local hardware tooling.

mod cli;
mod command_registry;
mod commands;
mod evidence;
mod obligation;
mod output;
mod process;
mod proof;
mod site;
mod suites;
mod three_body_actions;
mod workspace;

use clap::Parser;
use cli::{Cli, Command, DemoCommand, DoctorTarget, GlobalOpts, MakeTarget, ProveCommand};
use commands::check::CheckScope;

fn main() {
    let cli = Cli::parse_from(command_registry::normalize_compatibility_aliases(
        std::env::args_os().collect(),
    ));
    let opts = cli.global;
    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Command::Check(mut args) => {
            if let Some(scope) = args.scope.take() {
                if args.suite.is_some() {
                    Err("a check suite cannot be combined with a scoped check".into())
                } else {
                    match scope {
                        CheckScope::Catalog(args) => commands::catalog::run(args, &opts)
                            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
                        CheckScope::Plots(args) => {
                            commands::plots::run(args, &opts).map_err(|error| {
                                Box::new(std::io::Error::other(error)) as Box<dyn std::error::Error>
                            })
                        }
                        CheckScope::Pete => commands::pete_workload_check::run(&opts),
                        CheckScope::DeviceProtocols => commands::device_protocols::run(&opts),
                    }
                }
            } else {
                commands::check::run(args, &opts)
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            }
        }
        Command::Integrate => commands::integrate::run(&opts),
        Command::Ci(args) => commands::ci::run(args),
        Command::Make(args) => run_make(args.target, &opts),
        Command::Prove(mut args) => {
            if let Some(command) = args.command.take() {
                if args.proof.is_some()
                    || args.list
                    || args.verify_record.is_some()
                    || args.run_obligation
                {
                    Err("a structured proof operation cannot be combined with a proof target, --list, --verify, or --run-obligation".into())
                } else {
                    match command {
                        ProveCommand::AudioPlayback(args) => commands::audio::prove(
                            &opts,
                            &args.card_id,
                            args.device,
                            args.authorize_output,
                        ),
                        ProveCommand::BodyCoordination(args) => {
                            commands::body_coordination::run(args, &opts)
                        }
                        ProveCommand::Pete(args) => commands::pete_std_observe::run(args, &opts),
                        ProveCommand::Journey(args) => run_journey(args, &opts),
                        ProveCommand::TodoJourney(args) => commands::todo_journey::run(args, &opts),
                        ProveCommand::Evidence(evidence) => commands::evidence::run(evidence),
                    }
                }
            } else if args.list || args.verify_record.is_some() || args.run_obligation {
                commands::proofs::run(&args, opts.json)
            } else {
                commands::prove::run(*args, &opts)
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            }
        }
        Command::Doctor(args) => match args.target {
            DoctorTarget::Audio => commands::audio::list(&opts),
            DoctorTarget::Midi => commands::midi::list(&opts),
            _ => commands::doctor::run(args, &opts)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        },
        Command::Setup(args) => commands::setup::run(args, &opts),
    };

    if let Err(error) = result {
        eprintln!("xtask error: {error}");
        std::process::exit(1);
    }
}

fn run_journey(args: cli::DemoArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        DemoCommand::Workspace(args) => commands::workspace::run(&args, opts),
        DemoCommand::Std => commands::demo::run_std(opts),
        DemoCommand::Triple => commands::demo::run_triple(opts),
        DemoCommand::Patchbay(args) => commands::demo::run_patchbay(&args, opts),
        DemoCommand::BodyMembership => commands::demo::run_body_membership(opts),
        DemoCommand::Environment => commands::demo::run_environment(opts),
        DemoCommand::Prewake => commands::demo::run_prewake(opts),
        DemoCommand::TextLab => commands::demo::run_text_lab(opts),
        DemoCommand::Toggle => commands::toggle::run(),
        DemoCommand::LightSwitch(args) => commands::light_switch::run(args),
        DemoCommand::ButtonIndicator(args) => commands::button_indicator::run(args, opts),
        DemoCommand::Site => commands::toggle::run_site(),
        DemoCommand::Tongues => commands::tongues::run(opts),
        DemoCommand::TonguesResearch => commands::tongues::run_research(opts),
        DemoCommand::TonguesAnalysis => commands::tongues::run_analysis(opts),
        DemoCommand::NativeSpeech(args) => commands::native_speech::run(args, opts),
    }
}

fn run_make(target: MakeTarget, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match target {
        MakeTarget::Avr(args) => commands::avr::run(args, opts),
        MakeTarget::Browser => commands::browser::run(opts),
        MakeTarget::Body(args) => commands::body::run(args, opts),
        MakeTarget::Esp32Firmware(args) => commands::esp32_firmware::run(args, opts),
        MakeTarget::Host(args) => commands::host::run(args, opts),
        MakeTarget::Pico(mut args) => run_pico(opts, &mut args, false),
        MakeTarget::PicoLocal(mut args) => run_pico(opts, &mut args, true),
        MakeTarget::Conduitos(args) => commands::conduitos::run(args, opts)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        MakeTarget::Handbook(args) => commands::handbook::run_handbook(args),
        MakeTarget::PagesRoot(args) => commands::handbook::run_pages_root(args),
        MakeTarget::UnifontSubset(args) => commands::unifont_subset::run(args),
        MakeTarget::PaletteIcons(args) => commands::palette_icons::run(args),
        MakeTarget::StartupCue(args) => commands::audio::cue::render(opts, &args.output),
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
