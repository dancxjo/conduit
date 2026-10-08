mod birth_identity;
mod body_face_json;
mod body_product;
mod body_terminal;
#[cfg(unix)]
mod body_terminal_owner;
mod cli;
mod deployment_carrier;
mod diagnostics;
mod durable_host;
mod durable_host_control;
mod host_install;
mod host_rendezvous;
mod inspection;
mod native_package_install;
mod plot_diagram;
mod plot_source;
mod product_execution;
#[cfg(test)]
mod product_execution_tests;
mod release_obtain;
#[cfg(test)]
#[path = "rendezvous_relay.rs"]
mod rendezvous_relay;
mod report_artifact;
mod screen_free_birth;
mod source_expansion;
mod std_websocket_line;
#[cfg(test)]
mod two_std_line_tests;
#[cfg(test)]
mod v1_history;

use clap::Parser;
use std::io;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

fn current_state_dir() -> Result<PathBuf, String> {
    let value = std::env::var_os("CONDUIT_STATE_DIR")
        .ok_or("CONDUIT_STATE_DIR must name this installed Host's durable state")?;
    if value.is_empty() {
        return Err("CONDUIT_STATE_DIR must not be empty".into());
    }
    Ok(value.into())
}

fn enter_conduit() -> Result<(), String> {
    match std::env::var_os("CONDUIT_STATE_DIR") {
        Some(value) if value.is_empty() => Err("CONDUIT_STATE_DIR must not be empty".into()),
        Some(value) => {
            let state_dir = PathBuf::from(value);
            durable_host::has_current_body(&state_dir)?;
            enter_workspace()
        }
        None => enter_workspace(),
    }
}

fn enter_birth() -> Result<(), String> {
    enter_workspace()
}

fn enter_workspace() -> Result<(), String> {
    let conduit = std::env::current_exe()
        .map_err(|error| format!("cannot locate the installed Conduit entrance ({error})"))?;
    let mut command = workspace_command(&conduit)?;
    let executable = command.get_program().to_string_lossy().into_owned();
    let status = command.status().map_err(|error| {
        format!(
            "{executable} is unavailable ({error}); install the Conduit browser Host alongside the `conduit` product entrance"
        )
    })?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{executable} exited with {status}"))
}

fn workspace_command(conduit: &Path) -> Result<std::process::Command, String> {
    let executable = "conduit-browser-host";
    let application = conduit
        .parent()
        .ok_or("the installed Conduit entrance has no parent directory")?
        .join("conduit-workspace");
    if !application.join("index.html").is_file() {
        return Err(format!(
            "the admitted birth encounter is unavailable at {}; install it alongside the Conduit executables",
            application.display()
        ));
    }
    let mut command = std::process::Command::new(executable);
    command
        .arg("--application")
        .arg(application)
        .args(["--mount", "/workspace/"]);
    Ok(command)
}

use crate::report_artifact::{snapshot_from_execution, write_execution_artifacts, write_report};

fn run_with_placements(
    path: &str,
    placements_path: Option<&str>,
    report_path: Option<&Path>,
    artifact_directory: Option<&Path>,
    body_path: Option<&Path>,
    await_terminal: bool,
) -> Result<(), String> {
    let source = plot_source::load(Path::new(path))?;
    let plot = source.expand_entry()?;
    let body_product = body_path.map(body_product::prepare).transpose()?;
    let mut context = match body_product {
        Some(product) => product.context,
        None => product_execution::ProductExecutionContext::local_std()?,
    };
    let plan = context.plan(&plot, placements_path)?;
    let completion_policy = plan.completion_policy;
    let mut stdout = io::stdout().lock();
    let control = conduit_std_host::RunControl::default();
    let input_thread =
        if completion_policy == conduit_core::PlanCompletionPolicy::Live && !await_terminal {
            writeln!(
                stdout,
                "Play is live; press Enter or close standard input to cancel it explicitly"
            )
            .map_err(|error| error.to_string())?;
            let input_control = control.clone();
            Some(std::thread::spawn(move || {
                let mut line = String::new();
                let _ = io::stdin().lock().read_line(&mut line);
                let _ = input_control.request_stop(
                    conduit_std_host::RunControlRequestId::new("conduct-run/operator-interrupt")
                        .expect("static run-control identity is valid"),
                );
            }))
        } else {
            None
        };
    let execution = if await_terminal {
        context.execute(plan, &mut stdout)?
    } else {
        context.execute_attached(plan, &mut stdout, &control)?
    };
    if let Some(input_thread) = input_thread {
        input_thread
            .join()
            .map_err(|_| "interactive input thread failed".to_string())?;
    }
    if report_path.is_some() || artifact_directory.is_some() {
        let snapshot = snapshot_from_execution(
            execution.advertisements,
            execution.line_offers,
            vec![execution.plan],
            execution.observations,
        );
        if let Some(report_path) = report_path {
            write_report(report_path, &snapshot)?;
        }
        if let Some(artifact_directory) = artifact_directory {
            write_execution_artifacts(
                artifact_directory,
                &snapshot,
                &execution.active_plays,
                &execution.sign_identities,
            )?;
        }
    }
    Ok(())
}

fn main() {
    let command = cli::Cli::parse().command;
    let result = match command {
        None => enter_conduit(),
        Some(cli::Command::Run {
            plot,
            placements,
            report,
            artifacts,
            body,
            await_terminal,
        }) => run_with_placements(
            &plot.to_string_lossy(),
            placements.as_deref().map(Path::to_string_lossy).as_deref(),
            report.as_deref(),
            artifacts.as_deref(),
            body.as_deref(),
            await_terminal,
        ),
        Some(cli::Command::Host { command: None }) => current_state_dir().and_then(|state_dir| {
            durable_host::dispatch(cli::HostServiceCommand::Status {
                state_dir,
                json: false,
            })
        }),
        Some(cli::Command::Host {
            command: Some(command),
        }) => match command {
            cli::HostCommand::Install {
                target,
                catalog,
                catalog_id,
                mirror,
                cache,
                carrier_descriptors,
                carrier,
                minimum_generation,
                request,
                dry_run,
            } => host_install::run(host_install::InstallRequest {
                target: &target,
                catalog: &catalog,
                catalog_id: &catalog_id,
                mirror: &mirror,
                cache: &cache,
                carrier_descriptors: &carrier_descriptors,
                carrier: carrier.as_deref(),
                minimum_generation,
                realization_request: request.as_deref(),
                dry_run,
            }),
            cli::HostCommand::Service { command } => durable_host::dispatch(command),
            cli::HostCommand::Obtain {
                target,
                catalog,
                catalog_id,
                mirror,
                cache,
                minimum_generation,
            } => release_obtain::run(
                &target,
                &catalog,
                &catalog_id,
                &mirror,
                &cache,
                minimum_generation,
            ),
            cli::HostCommand::Carry { command } => deployment_carrier::run(command),
            cli::HostCommand::Rendezvous {
                state_dir,
                carrier,
                timeout_seconds,
                bind,
                public_url,
                tls_cert,
                tls_key,
                relay_descriptor,
                authorize_network,
            } => host_rendezvous::serve(
                &state_dir,
                carrier,
                timeout_seconds,
                host_rendezvous::SecureNetworkOptions {
                    bind,
                    public_url,
                    tls_cert,
                    tls_key,
                    authorize_network,
                },
                relay_descriptor.as_deref(),
            ),
        },
        Some(cli::Command::Body { command: None }) => {
            current_state_dir().and_then(|state_dir| durable_host::body_status(&state_dir, false))
        }
        Some(cli::Command::Body {
            command: Some(cli::BodyCommand::Status { state_dir, json }),
        }) => durable_host::body_status(&state_dir, json),
        Some(cli::Command::Body {
            command: Some(cli::BodyCommand::Face { state_dir, json }),
        }) => body_face_json::run(&state_dir, json),
        Some(cli::Command::Body {
            command: Some(cli::BodyCommand::SpokenMask { state_dir, command }),
        }) => durable_host_control::direct_spoken::run(&state_dir, command),
        Some(cli::Command::Body {
            command: Some(cli::BodyCommand::Start { state_dir, maximum_millis, todo_new_list }),
        }) => durable_host_control::start_owned_body(&state_dir, maximum_millis, todo_new_list),
        Some(cli::Command::Body {
            command: Some(cli::BodyCommand::Lull { state_dir }),
        }) => durable_host_control::lull_owned_body(&state_dir),
        Some(cli::Command::Body {
            command: Some(cli::BodyCommand::Terminal { state_dir, owner_show }),
        }) => {
            if owner_show {
                #[cfg(unix)]
                {
                    body_terminal_owner::run(
                        &state_dir,
                        &mut io::stdin().lock(),
                        &mut io::stdout().lock(),
                    )
                }
                #[cfg(not(unix))]
                {
                    Err("owner terminal attachment requires a local Unix Host".into())
                }
            } else {
                body_terminal::run(&state_dir, &mut io::stdin().lock(), &mut io::stdout().lock())
            }
        }
        Some(cli::Command::Body {
            command:
                Some(cli::BodyCommand::Invite {
                    state_dir,
                    ttl_seconds,
                    route_bind,
                    route_url,
                    route_tls_cert,
                    route_tls_key,
                    authorize_route,
                }),
        }) => match (route_bind, route_url, route_tls_cert, route_tls_key) {
            (None, None, None, None) if !authorize_route => {
                durable_host::issue_body_invitation(&state_dir, ttl_seconds)
            }
            (Some(bind), Some(url), Some(certificate), Some(private_key)) => {
                durable_host::serve_body_invitation_route(
                    &state_dir,
                    ttl_seconds,
                    bind,
                    &url,
                    &certificate,
                    &private_key,
                    authorize_route,
                )
            }
            _ => Err("a routed invitation requires --route-bind, --route-url, --route-tls-cert, --route-tls-key, and --authorize-route together".into()),
        },
        Some(cli::Command::Body {
            command:
                Some(cli::BodyCommand::BrowserWindow {
                    state_dir,
                    expected_host_id,
                    new_host_verifying_key,
                    maximum_millis,
                    authorize_window,
                }),
        }) => {
            if !authorize_window {
                Err("browser admission window requires --authorize-window".into())
            } else {
                let key = new_host_verifying_key
                    .as_deref()
                    .map(|text| {
                        serde_json::from_str::<[u8; 32]>(text)
                            .map_err(|error| format!("browser verifying key must be a 32-byte JSON array: {error}"))
                    })
                    .transpose();
                key.and_then(|key| {
                    durable_host_control::start_browser_window(
                        &state_dir,
                        &expected_host_id,
                        key,
                        maximum_millis,
                    )
                })
            }
        }
        Some(cli::Command::Body {
            command:
                Some(cli::BodyCommand::Join {
                    invitation,
                    state_dir,
                    authorize_join,
                }),
        }) => durable_host::join_body_over_route(&invitation, &state_dir, authorize_join),
        Some(cli::Command::Body {
            command:
                Some(cli::BodyCommand::Accept {
                    invitation,
                    state_dir,
                    authorize_join,
                }),
        }) => durable_host::accept_body_invitation(&invitation, &state_dir, authorize_join),
        Some(cli::Command::Body {
            command:
                Some(cli::BodyCommand::Admit {
                    request,
                    state_dir,
                    authorize_admission,
                }),
        }) => durable_host::admit_body_request(&request, &state_dir, authorize_admission),
        Some(cli::Command::Body {
            command:
                Some(cli::BodyCommand::CompleteJoin {
                    receipt,
                    state_dir,
                    authorize_membership,
                }),
        }) => durable_host::complete_body_join(&receipt, &state_dir, authorize_membership),
        Some(cli::Command::Body {
            command: Some(command),
        }) => match command {
            cli::BodyCommand::SpeechOptions { json } => {
                screen_free_birth::speech_options(json, &mut io::stdout().lock())
            }
            cli::BodyCommand::Birth { screen_free, state_dir, speech } => {
                if speech.speak && !screen_free {
                    Err("--speak requires --screen-free for Birth".into())
                } else if screen_free {
                    state_dir.map_or_else(current_state_dir, Ok).and_then(|state_dir| {
                        if speech.speak {
                            screen_free_birth::run_installed_spoken(&state_dir, &speech, &mut io::stdout().lock())
                        } else {
                            screen_free_birth::run_installed(
                                &state_dir,
                                &mut io::stdin().lock(),
                                &mut io::stdout().lock(),
                            )
                        }
                    })
                } else {
                    enter_birth()
                }
            }
            cli::BodyCommand::ScreenFree { state_dir, speech } => {
                state_dir.map_or_else(current_state_dir, Ok).and_then(|state_dir| {
                    if speech.speak {
                        screen_free_birth::run_retained_spoken(
                            &state_dir,
                            &speech,
                            &mut io::stdout().lock(),
                        )
                    } else {
                        screen_free_birth::run_retained(
                            &state_dir,
                            &mut io::stdin().lock(),
                            &mut io::stdout().lock(),
                        )
                    }
                })
            }
            cli::BodyCommand::Own { source, state_dir, name } => durable_host::run_body_owner(&source, &state_dir, &name),
            _ => unreachable!("durable Body operations are dispatched above"),
        },
        Some(cli::Command::Check { plot, json }) => match diagnostics::run(&plot, json) {
            Ok(true) => Ok(()),
            Ok(false) => std::process::exit(1),
            Err(error) => Err(error),
        },
        Some(cli::Command::Expand { plot, json }) => {
            source_expansion::run(&plot, json).map(|rendered| print!("{rendered}"))
        }
        Some(cli::Command::Diagram {
            plot,
            format,
            output,
        }) => plot_diagram::run(&plot, format, output.as_deref()),
        Some(cli::Command::Inspect { thing }) => inspection::inspect(&thing).map(|rendered| {
            print!("{rendered}");
        }),
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod product_entrance_tests {
    use super::*;

    #[test]
    fn resident_entrance_uses_the_workspace_browser_application() {
        let root = std::env::temp_dir().join(format!(
            "conduit-workspace-entrance-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let application = root.join("conduit-workspace");
        std::fs::create_dir_all(&application).unwrap();
        std::fs::write(application.join("index.html"), b"workspace").unwrap();

        let command = workspace_command(&root.join("conduit")).unwrap();
        assert_eq!(command.get_program(), "conduit-browser-host");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                std::ffi::OsStr::new("--application"),
                application.as_os_str(),
                std::ffi::OsStr::new("--mount"),
                std::ffi::OsStr::new("/workspace/"),
            ]
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
