use std::cell::RefCell;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::Parser;
use clap::error::ErrorKind;
use conduct::run_stream::{RUN_CHANNEL_CHUNK_MAX_BYTES, RunNdjsonState};
use conduct::{
    Arguments, CapsuleOperation, ColorChoice, DiagnosticFormat, InspectKind, Mode, OutputFormat,
    PackageOperation, SecondaryCommand,
};
use conduit_capsule::{
    ArtifactReference as CapsuleArtifactReference, CapsuleDocument, InlineDocument,
    MAXIMUM_AUXILIARY_BYTES, MAXIMUM_CAPSULE_DOCUMENT_BYTES, MAXIMUM_SOURCE_BYTES,
};
use conduit_compile::{
    CompileInput, ExactPlanDocument, InstalledProfile, MAXIMUM_COMPILE_INPUT_DOCUMENT_BYTES,
    compile_source,
};
use conduit_core::{ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy};
use conduit_diagnostics::{
    DIAGNOSTIC_SCHEMA_VERSION, DiagnosticSource, OwnedDiagnostic, TerminalColor, TerminalVerbosity,
    from_parse_error, from_resolution_error, from_runtime_error, render_terminal,
};
use conduit_inspect::{
    ArtifactKind, InspectLimits, RequestedKind, inspect_bytes, inspect_conformance_manifest_path,
    inspect_panel_path, read_bounded, read_stream_bounded,
};
use conduit_package::{
    PackageLimits, PackageManifest, PackageSignatureObservation, PackageTrustPolicy,
    decode_package, encode_package, validate_package_trust,
};
use conduit_panel::{MAXIMUM_PANEL_SOURCE_BYTES, parse, parse_with_root};
use conduit_runtime::{
    ExactExecutionReport, ExactRunContext, ExecutionSummary, Registry, ResolvedPanel,
    ResolvedPanelView, RunIo, RuntimeError, SchedulerReservation,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PresentationOptions {
    diagnostic_format: DiagnosticFormat,
    color: ColorChoice,
    verbose_diagnostics: bool,
}

enum RunOutcome {
    Exact(Box<ExactExecutionReport>),
    Compatibility(ExecutionSummary),
}

impl RunOutcome {
    const fn summary(&self) -> ExecutionSummary {
        match self {
            Self::Exact(report) => report.summary,
            Self::Compatibility(summary) => *summary,
        }
    }
}

fn presentation(arguments: &Arguments) -> PresentationOptions {
    PresentationOptions {
        diagnostic_format: arguments.diagnostic_format,
        color: arguments.color,
        verbose_diagnostics: arguments.verbose_diagnostics,
    }
}

#[derive(Serialize)]
struct FiniteResult<T> {
    schema: &'static str,
    schema_version: u16,
    operation: &'static str,
    result: T,
}

#[derive(Serialize)]
struct CheckResult {
    panel_version: u16,
    definitions: usize,
    root_nodes: usize,
    root_cords: usize,
}

#[derive(Serialize)]
struct PackageCreateResult {
    identity: String,
    objects: usize,
    embedded_objects: usize,
    package_bytes: usize,
    output: String,
}

#[derive(Serialize)]
struct PackageExtractResult {
    identity: String,
    extracted_objects: usize,
    extracted_bytes: u64,
    output_directory: String,
}

#[derive(Serialize)]
struct PackageVerifyResult {
    identity: String,
    selected_objects: usize,
    verified_observations: usize,
}

#[derive(Serialize)]
struct CapsuleSummary {
    identity: String,
    program_identity: String,
    source_revision: String,
    source_semantic_identity: String,
    artifact_references: usize,
    has_import_lock: bool,
    has_presentation: bool,
}

#[derive(Serialize)]
struct CapsuleCheckResult {
    summary: CapsuleSummary,
    panel_version: u16,
    definitions: usize,
    root_nodes: usize,
    root_cords: usize,
}

#[derive(Serialize)]
struct CapsuleDiffResult {
    same_capsule: bool,
    same_program: bool,
    same_source_revision: bool,
    same_source_semantics: bool,
    same_import_lock: bool,
    same_artifact_references: bool,
    same_presentation: bool,
}

impl PresentationOptions {
    fn scan(arguments: &[OsString]) -> Self {
        let mut options = Self::default();
        let mut index = 0;
        while index < arguments.len() {
            let value = arguments[index].to_string_lossy();
            match value.as_ref() {
                "--diagnostic-format=json" => {
                    options.diagnostic_format = DiagnosticFormat::Json;
                }
                "--diagnostic-format=human" => {
                    options.diagnostic_format = DiagnosticFormat::Human;
                }
                "--diagnostic-format" => {
                    if let Some(next) = arguments.get(index + 1).and_then(|value| value.to_str()) {
                        match next {
                            "json" => options.diagnostic_format = DiagnosticFormat::Json,
                            "human" => options.diagnostic_format = DiagnosticFormat::Human,
                            _ => {}
                        }
                        index += 1;
                    }
                }
                "--color=always" => options.color = ColorChoice::Always,
                "--color=never" => options.color = ColorChoice::Never,
                "--color=auto" => options.color = ColorChoice::Auto,
                "--color" => {
                    if let Some(next) = arguments.get(index + 1).and_then(|value| value.to_str()) {
                        match next {
                            "always" => options.color = ColorChoice::Always,
                            "never" => options.color = ColorChoice::Never,
                            "auto" => options.color = ColorChoice::Auto,
                            _ => {}
                        }
                        index += 1;
                    }
                }
                "--verbose-diagnostics" => options.verbose_diagnostics = true,
                _ => {}
            }
            index += 1;
        }
        options
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct EnvironmentPolicy {
    no_color: bool,
    clicolor: Option<OsString>,
    clicolor_force: Option<OsString>,
    term_dumb: bool,
}

impl EnvironmentPolicy {
    fn capture() -> Self {
        Self {
            no_color: env::var_os("NO_COLOR").is_some(),
            clicolor: env::var_os("CLICOLOR"),
            clicolor_force: env::var_os("CLICOLOR_FORCE"),
            term_dumb: env::var_os("TERM").is_some_and(|value| value == "dumb"),
        }
    }

    fn color_enabled(&self, choice: ColorChoice, stderr_is_terminal: bool) -> bool {
        match choice {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                if self.no_color {
                    return false;
                }
                if self
                    .clicolor_force
                    .as_ref()
                    .is_some_and(|value| value != "0")
                {
                    return true;
                }
                if self.term_dumb || self.clicolor.as_ref().is_some_and(|value| value == "0") {
                    return false;
                }
                stderr_is_terminal
            }
        }
    }

    fn status_enabled(
        &self,
        diagnostic_format: DiagnosticFormat,
        stderr_is_terminal: bool,
    ) -> bool {
        diagnostic_format == DiagnosticFormat::Human && stderr_is_terminal && !self.term_dumb
    }
}

#[derive(Debug)]
struct CliError {
    diagnostic: Box<OwnedDiagnostic>,
    sources: Vec<DiagnosticSource>,
    presentation: PresentationOptions,
}

#[derive(Debug)]
enum ParsedCommand {
    Execute(Arguments),
    Display(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Completion {
    Success,
    BrokenPipe,
}

fn main() -> ExitCode {
    if let Some(exit) = conduit_process::fixture_entrypoint() {
        return exit;
    }
    let raw_arguments: Vec<OsString> = env::args_os().skip(1).collect();
    let environment = EnvironmentPolicy::capture();
    let stderr_is_terminal = io::stderr().is_terminal();
    match run(raw_arguments, &environment, stderr_is_terminal) {
        Ok(Completion::Success | Completion::BrokenPipe) => ExitCode::SUCCESS,
        Err(error) => {
            emit_diagnostic(&error, &environment, stderr_is_terminal);
            ExitCode::from(2)
        }
    }
}

fn run(
    raw_arguments: Vec<OsString>,
    environment: &EnvironmentPolicy,
    stderr_is_terminal: bool,
) -> Result<Completion, CliError> {
    let scanned_presentation = PresentationOptions::scan(&raw_arguments);
    let arguments = match parse_arguments(raw_arguments, scanned_presentation)? {
        ParsedCommand::Execute(arguments) => arguments,
        ParsedCommand::Display(output) => {
            return write_primary(output.as_bytes(), scanned_presentation);
        }
    };
    let presentation = presentation(&arguments);
    validate_output_format(&arguments, presentation)?;
    if let Some(secondary) = &arguments.secondary {
        return match secondary {
            SecondaryCommand::Inspect(inspect) => run_inspect(
                &arguments,
                inspect,
                presentation,
                environment,
                stderr_is_terminal,
            ),
            SecondaryCommand::Compile(compile) => run_compile(
                &arguments,
                compile,
                presentation,
                environment,
                stderr_is_terminal,
            ),
            SecondaryCommand::Package(package) => run_package(
                &arguments,
                &package.operation,
                presentation,
                environment,
                stderr_is_terminal,
            ),
            SecondaryCommand::Capsule(capsule) => run_capsule(
                &arguments,
                &capsule.operation,
                presentation,
                environment,
                stderr_is_terminal,
            ),
        };
    }
    let started = Instant::now();

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let document_id = arguments
        .panel
        .as_deref()
        .filter(|path| path.as_os_str() != "-")
        .map_or_else(|| "stdin".to_owned(), |path| path.display().to_string());
    let status_enabled = !arguments.quiet
        && arguments.format == OutputFormat::Human
        && environment.status_enabled(arguments.diagnostic_format, stderr_is_terminal);
    emit_status(status_enabled, "Checking", &document_id);

    let source = match arguments.panel.as_deref() {
        None => read_source(&mut stdin, "stdin").map_err(|message| {
            cli_error(
                simple_diagnostic("CND-IO-001", &message),
                presentation,
                vec![],
            )
        })?,
        Some(path) if path.as_os_str() == "-" => {
            read_source(&mut stdin, "stdin").map_err(|message| {
                cli_error(
                    simple_diagnostic("CND-IO-001", &message),
                    presentation,
                    vec![],
                )
            })?
        }
        Some(path) => {
            let bytes = read_bounded(path, MAXIMUM_PANEL_SOURCE_BYTES as u64).map_err(|error| {
                let (code, message) = if error.code == "CND-INSP-005" {
                    (
                        "CND-SEC-001",
                        format!("panel source byte limit exceeded: {}", path.display()),
                    )
                } else {
                    (
                        "CND-IO-001",
                        format!("cannot read {}: {error}", path.display()),
                    )
                };
                cli_error(simple_diagnostic(code, &message), presentation, vec![])
            })?;
            String::from_utf8(bytes).map_err(|_| {
                cli_error(
                    simple_diagnostic("CND-SRC-001", "panel source is not valid UTF-8"),
                    presentation,
                    vec![],
                )
            })?
        }
    };
    let source_document = DiagnosticSource::new(document_id.clone(), source.as_bytes());
    let panel = parse(&source).map_err(|error| {
        cli_error(
            from_parse_error(&error, &source_document),
            presentation,
            vec![source_document.clone()],
        )
    })?;

    let registry = if arguments.compatibility_demo {
        Registry::compatibility_demo()
    } else {
        let mut registry = Registry::hosted_primitives();
        conduit_media::register_deterministic_media_providers(&mut registry).map_err(|error| {
            cli_error(
                simple_diagnostic(error.code, &error.message),
                presentation,
                vec![source_document.clone()],
            )
        })?;
        conduit_media::register_deterministic_codec_providers(&mut registry).map_err(|error| {
            cli_error(
                simple_diagnostic(error.code, &error.message),
                presentation,
                vec![source_document.clone()],
            )
        })?;
        conduit_learned::register_deterministic_inference_provider(&mut registry).map_err(
            |error| {
                cli_error(
                    simple_diagnostic(error.code, &error.message),
                    presentation,
                    vec![source_document.clone()],
                )
            },
        )?;
        conduit_filesystem::register_hosted_file_read_provider(&mut registry).map_err(|error| {
            cli_error(
                simple_diagnostic(error.code, &error.message),
                presentation,
                vec![source_document.clone()],
            )
        })?;
        if arguments.enable_file_write {
            conduit_filesystem::register_hosted_file_write_provider(&mut registry).map_err(
                |error| {
                    cli_error(
                        simple_diagnostic(error.code, &error.message),
                        presentation,
                        vec![source_document.clone()],
                    )
                },
            )?;
        }
        if arguments.enable_file_watch {
            conduit_filesystem::register_hosted_file_watch_provider(&mut registry).map_err(
                |error| {
                    cli_error(
                        simple_diagnostic(error.code, &error.message),
                        presentation,
                        vec![source_document.clone()],
                    )
                },
            )?;
        }
        if arguments.enable_storage_cache {
            conduit_cache::register_hosted_cache_provider(&mut registry).map_err(|error| {
                cli_error(
                    simple_diagnostic(error.code, &error.message),
                    presentation,
                    vec![source_document.clone()],
                )
            })?;
        }
        if arguments.enable_process_exec {
            conduit_process::register_hosted_process_provider(&mut registry).map_err(|error| {
                cli_error(
                    simple_diagnostic(error.code, &error.message),
                    presentation,
                    vec![source_document.clone()],
                )
            })?;
        }
        if arguments.enable_socket_loopback {
            conduit_socket::register_hosted_socket_providers(&mut registry).map_err(|error| {
                cli_error(
                    simple_diagnostic(error.code, &error.message),
                    presentation,
                    vec![source_document.clone()],
                )
            })?;
        }
        if arguments.enable_http_client_loopback {
            conduit_http::register_hosted_http_client_provider(&mut registry).map_err(|error| {
                cli_error(
                    simple_diagnostic(error.code, &error.message),
                    presentation,
                    vec![source_document.clone()],
                )
            })?;
        }
        conduit_http::register_hosted_http_provider(&mut registry).map_err(|error| {
            cli_error(
                simple_diagnostic(error.code, &error.message),
                presentation,
                vec![source_document.clone()],
            )
        })?;
        registry
    };
    let installed_profile = if arguments.mode() == Mode::Run && !arguments.compatibility_demo {
        Some(
            InstalledProfile::observe_registry(&source, &registry).map_err(|error| {
                cli_error(
                    from_runtime_error(&error),
                    presentation,
                    vec![source_document.clone()],
                )
            })?,
        )
    } else {
        None
    };
    let explicit_input = if let Some(input_path) = &arguments.compile_input {
        let input_bytes = read_bounded(input_path, MAXIMUM_COMPILE_INPUT_DOCUMENT_BYTES)
            .map_err(|error| inspection_error(error, presentation))?;
        Some(
            serde_json::from_slice::<CompileInput>(&input_bytes).map_err(|_| {
                package_error(
                    "CND-CMP-002",
                    "compile input is not valid conduit.compile-input JSON",
                    presentation,
                )
            })?,
        )
    } else {
        None
    };
    let compiled = explicit_input
        .as_ref()
        .or_else(|| installed_profile.as_ref().map(|profile| &profile.input))
        .map(|input| {
            compile_source(&source, input).map_err(|error| {
                cli_error(
                    simple_diagnostic(error.code(), &error.to_string()),
                    presentation,
                    vec![source_document.clone()],
                )
            })
        })
        .transpose()?;

    emit_status(status_enabled, "Resolving", &document_id);
    let resolved = registry.resolve(&panel).map_err(|error| {
        cli_error(
            from_resolution_error(&error),
            presentation,
            vec![source_document.clone()],
        )
    })?;
    if arguments.verbose > 0 {
        let view = resolved.view();
        emit_status(
            status_enabled,
            "Resolved",
            &format!("{} nodes, {} cords", view.nodes.len(), view.cords.len()),
        );
        if arguments.verbose > 1 {
            emit_status(
                status_enabled,
                "Selected",
                &format!(
                    "{} output for {}",
                    output_format_name(arguments.format),
                    mode_name(arguments.mode())
                ),
            );
        }
    }

    match arguments.mode() {
        Mode::Check => {
            let completion = match arguments.format {
                OutputFormat::Human => write_primary(
                    format!(
                        "ok: panel v{}; {} definitions; {} root nodes; {} root cords\n",
                        panel.version,
                        panel.definitions.len(),
                        panel.nodes.len(),
                        panel.cords.len()
                    )
                    .as_bytes(),
                    presentation,
                )?,
                OutputFormat::Json => write_json_primary(
                    &FiniteResult {
                        schema: "conduit.result",
                        schema_version: 0,
                        operation: "check",
                        result: CheckResult {
                            panel_version: panel.version,
                            definitions: panel.definitions.len(),
                            root_nodes: panel.nodes.len(),
                            root_cords: panel.cords.len(),
                        },
                    },
                    presentation,
                )?,
                OutputFormat::Ndjson => unreachable!("validated above"),
            };
            if completion == Completion::BrokenPipe {
                return Ok(completion);
            }
            emit_finished(
                status_enabled,
                "check",
                started.elapsed(),
                &format!(
                    "{} root nodes, {} root cords",
                    panel.nodes.len(),
                    panel.cords.len()
                ),
            );
        }
        Mode::Explain => {
            let completion = match arguments.format {
                OutputFormat::Human => write_primary(resolved.explain().as_bytes(), presentation)?,
                OutputFormat::Json => write_json_primary(
                    &FiniteResult::<ResolvedPanelView> {
                        schema: "conduit.result",
                        schema_version: 0,
                        operation: "explain",
                        result: resolved.view(),
                    },
                    presentation,
                )?,
                OutputFormat::Ndjson => unreachable!("validated above"),
            };
            if completion == Completion::BrokenPipe {
                return Ok(completion);
            }
            emit_finished(
                status_enabled,
                "explain",
                started.elapsed(),
                &format!(
                    "{} root nodes, {} root cords",
                    panel.nodes.len(),
                    panel.cords.len()
                ),
            );
        }
        Mode::Run => {
            emit_status(status_enabled, "Running", &document_id);
            let stdout = io::stdout();
            let stderr = io::stderr();
            let summary = match arguments.format {
                OutputFormat::Human => {
                    let mut output = ObservedWriter::new(stdout.lock());
                    let summary = {
                        let mut error = stderr.lock();
                        let mut display = stdout.lock();
                        execute_run(
                            &resolved,
                            compiled.as_ref(),
                            installed_profile.as_ref(),
                            arguments.compatibility_demo,
                            &mut RunIo {
                                input: &mut stdin,
                                output: &mut output,
                                error: &mut error,
                                display: &mut display,
                            },
                        )
                    };
                    if output.broken_pipe {
                        return Ok(Completion::BrokenPipe);
                    }
                    if let Some(failure) = output.failure.take() {
                        return Err(output_failure(&failure, presentation));
                    }
                    summary
                }
                OutputFormat::Ndjson => {
                    let stream =
                        RefCell::new(RunNdjsonState::new(ObservedWriter::new(stdout.lock())));
                    let summary = {
                        let mut output = RunNdjsonChannelWriter::new(&stream, "stdout");
                        let mut error = RunNdjsonChannelWriter::new(&stream, "stderr");
                        let mut display = RunNdjsonChannelWriter::new(&stream, "display");
                        execute_run(
                            &resolved,
                            compiled.as_ref(),
                            installed_profile.as_ref(),
                            arguments.compatibility_demo,
                            &mut RunIo {
                                input: &mut stdin,
                                output: &mut output,
                                error: &mut error,
                                display: &mut display,
                            },
                        )
                    };
                    let mut stream = stream.into_inner();
                    if stream.inner.broken_pipe {
                        return Ok(Completion::BrokenPipe);
                    }
                    if let Some(failure) = stream.inner.failure.take() {
                        return Err(output_failure(&failure, presentation));
                    }
                    if let Ok(outcome) = &summary {
                        if let RunOutcome::Exact(report) = outcome {
                            for evidence in &report.evidence {
                                if let Err(error) = stream.write_exact_evidence(evidence) {
                                    if stream.inner.broken_pipe {
                                        return Ok(Completion::BrokenPipe);
                                    }
                                    return Err(output_error(error, presentation));
                                }
                            }
                        }
                        if let Err(error) = stream.write_summary(outcome.summary()) {
                            if stream.inner.broken_pipe {
                                return Ok(Completion::BrokenPipe);
                            }
                            return Err(output_error(error, presentation));
                        }
                    }
                    summary
                }
                OutputFormat::Json => unreachable!("validated above"),
            };
            let outcome = summary.map_err(|error| {
                cli_error(
                    from_runtime_error(&error),
                    presentation,
                    vec![source_document],
                )
            })?;
            let summary = outcome.summary();
            emit_finished(
                status_enabled,
                "run",
                started.elapsed(),
                &format!(
                    "{} nodes, {} cords",
                    summary.nodes_completed, summary.cords_conducted
                ),
            );
        }
    }
    Ok(Completion::Success)
}

fn execute_run(
    resolved: &ResolvedPanel<'_>,
    compiled: Option<&ExactPlanDocument>,
    installed_profile: Option<&InstalledProfile>,
    compatibility_demo: bool,
    io: &mut RunIo<'_>,
) -> Result<RunOutcome, RuntimeError> {
    let Some(document) = compiled else {
        if compatibility_demo {
            return resolved.run_batch(io).map(RunOutcome::Compatibility);
        }
        return Err(RuntimeError::new(
            "CND-RUN-011",
            "production run requires an explicit --compile-input exact binding snapshot",
        ));
    };
    let arena = bumpalo::Bump::new();
    let plan = document
        .as_plan(&arena)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let installed_profile = installed_profile
        .ok_or_else(|| RuntimeError::new("CND-RUN-007", "installed provider profile is absent"))?;
    let bindings = installed_profile.bindings(&plan)?;
    let grant_observations = installed_profile.grant_observations(&plan)?;
    resolved
        .run_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 1,
                run_id: conduit_core::Id("conduit/conduct-run"),
                validation: conduit_core::PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 256,
                    max_tick: 512,
                    max_consecutive_yields: 8,
                    max_events: if plan.nodes.len() > 4 { 256 } else { 128 },
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
                grant_observations: &grant_observations,
            },
            io,
        )
        .map(|report| RunOutcome::Exact(Box::new(report)))
}

fn parse_arguments(
    raw_arguments: Vec<OsString>,
    presentation: PresentationOptions,
) -> Result<ParsedCommand, CliError> {
    let all_arguments = std::iter::once(OsString::from("conduct")).chain(raw_arguments);
    match Arguments::try_parse_from(all_arguments) {
        Ok(arguments) => Ok(ParsedCommand::Execute(arguments)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            Ok(ParsedCommand::Display(error.to_string()))
        }
        Err(error) => Err(argument_error(error, presentation)),
    }
}

fn argument_error(error: clap::Error, presentation: PresentationOptions) -> CliError {
    let rendered = error.to_string();
    let mut lines = rendered.lines();
    let message = lines
        .next()
        .unwrap_or("invalid command line")
        .strip_prefix("error: ")
        .unwrap_or("invalid command line");
    let mut diagnostic = simple_diagnostic("CND-CLI-001", message);
    diagnostic.notes = lines
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("For more information"))
        .map(str::to_owned)
        .collect();
    diagnostic.help = Some("run `conduct --help` for the canonical invocation".to_owned());
    cli_error(diagnostic, presentation, vec![])
}

fn validate_output_format(
    arguments: &Arguments,
    presentation: PresentationOptions,
) -> Result<(), CliError> {
    if arguments.compatibility_demo
        && (arguments.secondary.is_some() || arguments.mode() != Mode::Run)
    {
        return Err(cli_error(
            simple_diagnostic(
                "CND-CLI-004",
                "--compatibility-demo is available only with run mode",
            ),
            presentation,
            vec![],
        ));
    }
    if arguments.compile_input.is_some() && arguments.secondary.is_some() {
        return Err(cli_error(
            simple_diagnostic(
                "CND-CLI-004",
                "--compile-input is available only with check, explain, or run",
            ),
            presentation,
            vec![],
        ));
    }
    if arguments.secondary.is_some() && (arguments.check || arguments.explain || arguments.run) {
        return Err(cli_error(
            simple_diagnostic(
                "CND-CLI-004",
                "secondary operations cannot be combined with check, explain, or run",
            ),
            presentation,
            vec![],
        ));
    }
    let message = if arguments.secondary.is_some() {
        match arguments.format {
            OutputFormat::Human | OutputFormat::Json => None,
            OutputFormat::Ndjson => {
                Some("finite secondary operations use `--format=human` or `--format=json`")
            }
        }
    } else {
        match (arguments.mode(), arguments.format) {
            (Mode::Check | Mode::Explain, OutputFormat::Ndjson) => {
                Some("finite check and explain operations use `--format=human` or `--format=json`")
            }
            (Mode::Run, OutputFormat::Json) => {
                Some("streaming run output uses `--format=human` or `--format=ndjson`")
            }
            _ => None,
        }
    };
    if let Some(message) = message {
        let mut diagnostic = simple_diagnostic("CND-CLI-003", message);
        diagnostic.help = Some(
            "diagnostic encoding remains independently selected by `--diagnostic-format`"
                .to_owned(),
        );
        return Err(cli_error(diagnostic, presentation, vec![]));
    }
    Ok(())
}

fn run_inspect(
    arguments: &Arguments,
    inspect: &conduct::InspectArguments,
    presentation: PresentationOptions,
    environment: &EnvironmentPolicy,
    stderr_is_terminal: bool,
) -> Result<Completion, CliError> {
    let started = Instant::now();
    let limits = InspectLimits::default();
    let status_enabled = !arguments.quiet
        && arguments.format == OutputFormat::Human
        && environment.status_enabled(arguments.diagnostic_format, stderr_is_terminal);
    let label = inspect.artifact.display().to_string();
    emit_status(status_enabled, "Inspecting", &label);
    let requested = requested_inspect_kind(inspect.kind);
    let extension = inspect
        .artifact
        .extension()
        .and_then(|value| value.to_str());
    let (bytes, local_path) = if inspect.artifact.as_os_str() == "-" {
        let stdin = io::stdin();
        let mut stdin = stdin.lock();
        (
            read_stream_bounded(&mut stdin, limits.max_input_bytes)
                .map_err(|error| inspection_error(error, presentation))?,
            None,
        )
    } else {
        (
            read_bounded(&inspect.artifact, limits.max_input_bytes)
                .map_err(|error| inspection_error(error, presentation))?,
            Some(inspect.artifact.as_path()),
        )
    };
    let mut report = inspect_bytes(&bytes, requested, extension, limits)
        .map_err(|error| inspection_error(error, presentation))?;
    if report.kind == ArtifactKind::PanelSource {
        if let Some(path) = local_path {
            report = inspect_panel_path(path, limits)
                .map_err(|error| inspection_error(error, presentation))?;
        }
    } else if report.kind == ArtifactKind::ConformanceManifest {
        if let Some(path) = local_path {
            report = inspect_conformance_manifest_path(path, limits)
                .map_err(|error| inspection_error(error, presentation))?;
        }
    }
    if arguments.verbose > 0 {
        emit_status(
            status_enabled,
            "Identified",
            &format!("{} v{}", report.kind.as_str(), report.artifact_version),
        );
    }
    let completion = match arguments.format {
        OutputFormat::Human => write_primary(report.render_human().as_bytes(), presentation)?,
        OutputFormat::Json => write_json_primary(
            &FiniteResult {
                schema: "conduit.result",
                schema_version: 0,
                operation: "inspect",
                result: report,
            },
            presentation,
        )?,
        OutputFormat::Ndjson => unreachable!("validated above"),
    };
    if completion == Completion::BrokenPipe {
        return Ok(completion);
    }
    emit_finished(status_enabled, "inspect", started.elapsed(), &label);
    Ok(Completion::Success)
}

const fn requested_inspect_kind(kind: InspectKind) -> RequestedKind {
    match kind {
        InspectKind::Auto => RequestedKind::Auto,
        InspectKind::Panel => RequestedKind::Panel,
        InspectKind::LoweredSource => RequestedKind::LoweredSource,
        InspectKind::ExecutionPlan => RequestedKind::ExecutionPlan,
        InspectKind::Evidence => RequestedKind::Evidence,
        InspectKind::Diagnostic => RequestedKind::Diagnostic,
        InspectKind::Conformance => RequestedKind::Conformance,
        InspectKind::Package => RequestedKind::Package,
    }
}

fn run_compile(
    arguments: &Arguments,
    compile: &conduct::CompileArguments,
    presentation: PresentationOptions,
    environment: &EnvironmentPolicy,
    stderr_is_terminal: bool,
) -> Result<Completion, CliError> {
    let started = Instant::now();
    let status_enabled = !arguments.quiet
        && arguments.format == OutputFormat::Human
        && environment.status_enabled(arguments.diagnostic_format, stderr_is_terminal);
    let document_id = if compile.panel.as_os_str() == "-" {
        "stdin".to_owned()
    } else {
        compile.panel.display().to_string()
    };
    emit_status(status_enabled, "Compiling", &document_id);
    let input_bytes = read_bounded(&compile.input, MAXIMUM_COMPILE_INPUT_DOCUMENT_BYTES)
        .map_err(|error| inspection_error(error, presentation))?;
    let input: CompileInput = serde_json::from_slice(&input_bytes).map_err(|_| {
        package_error(
            "CND-CMP-002",
            "compile input is not valid conduit.compile-input JSON",
            presentation,
        )
    })?;
    input.validate_source_limits().map_err(|error| {
        cli_error(
            simple_diagnostic(error.code(), &error.to_string()),
            presentation,
            vec![],
        )
    })?;
    let source_bytes = if compile.panel.as_os_str() == "-" {
        let stdin = io::stdin();
        read_stream_bounded(
            &mut stdin.lock(),
            input.source_limits.maximum_entry_source_bytes,
        )
        .map_err(|error| compile_source_read_error(error, presentation))?
    } else {
        read_bounded(
            &compile.panel,
            input.source_limits.maximum_entry_source_bytes,
        )
        .map_err(|error| compile_source_read_error(error, presentation))?
    };
    let source = String::from_utf8(source_bytes).map_err(|_| {
        cli_error(
            simple_diagnostic("CND-CMP-003", "entry source is not valid UTF-8"),
            presentation,
            vec![],
        )
    })?;
    let source_document = DiagnosticSource::new(document_id.clone(), source.as_bytes());
    parse_with_root(&source, input.selected_root.as_deref()).map_err(|error| {
        cli_error(
            from_parse_error(&error, &source_document),
            presentation,
            vec![source_document.clone()],
        )
    })?;
    let plan = compile_source(&source, &input).map_err(|error| {
        cli_error(
            simple_diagnostic(error.code(), &error.to_string()),
            presentation,
            vec![source_document],
        )
    })?;
    let completion = match arguments.format {
        OutputFormat::Human => {
            let mut bytes = serde_json::to_vec_pretty(&plan).map_err(|error| {
                output_failure(&format!("cannot encode exact plan: {error}"), presentation)
            })?;
            bytes.push(b'\n');
            write_primary(&bytes, presentation)?
        }
        OutputFormat::Json => write_json_primary(
            &FiniteResult {
                schema: "conduit.result",
                schema_version: 0,
                operation: "compile",
                result: plan,
            },
            presentation,
        )?,
        OutputFormat::Ndjson => unreachable!("validated above"),
    };
    if completion == Completion::BrokenPipe {
        return Ok(completion);
    }
    emit_finished(status_enabled, "compile", started.elapsed(), &document_id);
    Ok(Completion::Success)
}

fn run_package(
    arguments: &Arguments,
    operation: &PackageOperation,
    presentation: PresentationOptions,
    environment: &EnvironmentPolicy,
    stderr_is_terminal: bool,
) -> Result<Completion, CliError> {
    let started = Instant::now();
    let limits = PackageLimits::default();
    let status_enabled = !arguments.quiet
        && arguments.format == OutputFormat::Human
        && environment.status_enabled(arguments.diagnostic_format, stderr_is_terminal);
    match operation {
        PackageOperation::Create(create) => {
            emit_status(
                status_enabled,
                "Packaging",
                &create.output.display().to_string(),
            );
            let manifest_bytes =
                read_bounded(&create.manifest, u64::from(limits.maximum_manifest_bytes))
                    .map_err(|error| package_input_error(error, presentation))?;
            let manifest: PackageManifest =
                serde_json::from_slice(&manifest_bytes).map_err(|_| {
                    package_error(
                        conduit_package::PackageReason::MalformedManifest.code(),
                        "package manifest is not valid conduit.package JSON",
                        presentation,
                    )
                })?;
            manifest
                .validate(limits)
                .map_err(|error| package_library_error(error, presentation))?;
            let mut blobs = std::collections::BTreeMap::new();
            for binding in &create.blobs {
                let (digest, path) = binding.split_once('=').ok_or_else(|| {
                    package_error(
                        "CND-PKG-003",
                        "each --blob must be SHA256=PATH",
                        presentation,
                    )
                })?;
                if blobs.contains_key(digest) {
                    return Err(package_error(
                        "CND-PKG-003",
                        "duplicate --blob digest",
                        presentation,
                    ));
                }
                let bytes = read_bounded(std::path::Path::new(path), limits.maximum_object_bytes)
                    .map_err(|error| package_input_error(error, presentation))?;
                blobs.insert(digest.to_owned(), bytes);
            }
            let package = encode_package(&manifest, &blobs, limits)
                .map_err(|error| package_library_error(error, presentation))?;
            write_new_file(&create.output, &package)
                .map_err(|message| package_error("CND-IO-002", &message, presentation))?;
            let result = PackageCreateResult {
                identity: manifest.identity.clone(),
                objects: manifest.objects.len(),
                embedded_objects: blobs.len(),
                package_bytes: package.len(),
                output: create.output.display().to_string(),
            };
            let completion = match arguments.format {
                OutputFormat::Human => write_primary(
                    format!(
                        "created {}: {}; {} objects, {} embedded, {} bytes\n",
                        result.output,
                        result.identity,
                        result.objects,
                        result.embedded_objects,
                        result.package_bytes
                    )
                    .as_bytes(),
                    presentation,
                )?,
                OutputFormat::Json => write_json_primary(
                    &FiniteResult {
                        schema: "conduit.result",
                        schema_version: 0,
                        operation: "package-create",
                        result,
                    },
                    presentation,
                )?,
                OutputFormat::Ndjson => unreachable!("validated above"),
            };
            if completion == Completion::BrokenPipe {
                return Ok(completion);
            }
            emit_finished(
                status_enabled,
                "package-create",
                started.elapsed(),
                &create.output.display().to_string(),
            );
        }
        PackageOperation::Verify(verify) => {
            emit_status(
                status_enabled,
                "Verifying",
                &verify.package.display().to_string(),
            );
            let bytes = read_bounded(&verify.package, limits.maximum_package_bytes)
                .map_err(|error| package_input_error(error, presentation))?;
            let package = decode_package(&bytes, limits)
                .map_err(|error| package_library_error(error, presentation))?;
            let policy_bytes =
                read_bounded(&verify.policy, u64::from(limits.maximum_manifest_bytes))
                    .map_err(|error| package_input_error(error, presentation))?;
            let policy: PackageTrustPolicy =
                serde_json::from_slice(&policy_bytes).map_err(|_| {
                    package_error(
                        "CND-PKG-003",
                        "package trust policy is not valid JSON",
                        presentation,
                    )
                })?;
            let observations_bytes = read_bounded(
                &verify.observations,
                u64::from(limits.maximum_manifest_bytes),
            )
            .map_err(|error| package_input_error(error, presentation))?;
            let observations: Vec<PackageSignatureObservation> =
                serde_json::from_slice(&observations_bytes).map_err(|_| {
                    package_error(
                        "CND-PKG-003",
                        "package trust observations are not a valid JSON array",
                        presentation,
                    )
                })?;
            validate_package_trust(&package.manifest, &policy, &observations, limits)
                .map_err(|error| package_library_error(error, presentation))?;
            let selected_objects = package
                .manifest
                .objects
                .iter()
                .filter(|object| policy.roles.is_empty() || policy.roles.contains(&object.role))
                .count();
            let result = PackageVerifyResult {
                identity: package.manifest.identity,
                selected_objects,
                verified_observations: observations
                    .iter()
                    .filter(|observation| observation.verified)
                    .count(),
            };
            let completion = match arguments.format {
                OutputFormat::Human => write_primary(
                    format!(
                        "verified {}: {}; {} selected objects, {} verified observations\n",
                        verify.package.display(),
                        result.identity,
                        result.selected_objects,
                        result.verified_observations
                    )
                    .as_bytes(),
                    presentation,
                )?,
                OutputFormat::Json => write_json_primary(
                    &FiniteResult {
                        schema: "conduit.result",
                        schema_version: 0,
                        operation: "package-verify",
                        result,
                    },
                    presentation,
                )?,
                OutputFormat::Ndjson => unreachable!("validated above"),
            };
            if completion == Completion::BrokenPipe {
                return Ok(completion);
            }
            emit_finished(
                status_enabled,
                "package-verify",
                started.elapsed(),
                &verify.package.display().to_string(),
            );
        }
        PackageOperation::Extract(extract) => {
            emit_status(
                status_enabled,
                "Extracting",
                &extract.package.display().to_string(),
            );
            let bytes = read_bounded(&extract.package, limits.maximum_package_bytes)
                .map_err(|error| package_input_error(error, presentation))?;
            let package = decode_package(&bytes, limits)
                .map_err(|error| package_library_error(error, presentation))?;
            let extracted_bytes = package.embedded_bytes();
            let paths = package
                .extract_to(&extract.output_dir, limits)
                .map_err(|error| package_library_error(error, presentation))?;
            let result = PackageExtractResult {
                identity: package.manifest.identity,
                extracted_objects: paths.len(),
                extracted_bytes,
                output_directory: extract.output_dir.display().to_string(),
            };
            let completion = match arguments.format {
                OutputFormat::Human => write_primary(
                    format!(
                        "extracted {}: {}; {} objects, {} bytes\n",
                        result.output_directory,
                        result.identity,
                        result.extracted_objects,
                        result.extracted_bytes
                    )
                    .as_bytes(),
                    presentation,
                )?,
                OutputFormat::Json => write_json_primary(
                    &FiniteResult {
                        schema: "conduit.result",
                        schema_version: 0,
                        operation: "package-extract",
                        result,
                    },
                    presentation,
                )?,
                OutputFormat::Ndjson => unreachable!("validated above"),
            };
            if completion == Completion::BrokenPipe {
                return Ok(completion);
            }
            emit_finished(
                status_enabled,
                "package-extract",
                started.elapsed(),
                &extract.output_dir.display().to_string(),
            );
        }
    }
    Ok(Completion::Success)
}

fn run_capsule(
    arguments: &Arguments,
    operation: &CapsuleOperation,
    presentation: PresentationOptions,
    environment: &EnvironmentPolicy,
    stderr_is_terminal: bool,
) -> Result<Completion, CliError> {
    let started = Instant::now();
    let status_enabled = !arguments.quiet
        && arguments.format == OutputFormat::Human
        && environment.status_enabled(arguments.diagnostic_format, stderr_is_terminal);
    let (operation_name, subject, human, result) = match operation {
        CapsuleOperation::Pack(pack) => {
            let source = read_bounded(&pack.panel, MAXIMUM_SOURCE_BYTES as u64)
                .map_err(|error| package_input_error(error, presentation))?;
            let source = String::from_utf8(source).map_err(|_| {
                capsule_cli_error("CND-CAP-003", "capsule panel is not UTF-8", presentation)
            })?;
            let import_lock = pack
                .lock
                .as_deref()
                .map(|path| {
                    read_inline_document(
                        path,
                        "application/vnd.conduit.contract-lock+json",
                        presentation,
                    )
                })
                .transpose()?;
            let presentation_document = pack
                .presentation
                .as_deref()
                .map(|path| {
                    read_inline_document(
                        path,
                        "application/vnd.conduit.presentation+json",
                        presentation,
                    )
                })
                .transpose()?;
            let references = pack
                .references
                .as_deref()
                .map(|path| {
                    let bytes = read_bounded(path, MAXIMUM_AUXILIARY_BYTES as u64)
                        .map_err(|error| package_input_error(error, presentation))?;
                    serde_json::from_slice::<Vec<CapsuleArtifactReference>>(&bytes).map_err(|_| {
                        capsule_cli_error(
                            "CND-CAP-005",
                            "capsule references are not a valid JSON array",
                            presentation,
                        )
                    })
                })
                .transpose()?
                .unwrap_or_default();
            let document =
                CapsuleDocument::new(source, import_lock, presentation_document, references)
                    .map_err(|error| capsule_library_error(error, presentation))?;
            let bytes = serde_json::to_vec_pretty(&document).map_err(|_| {
                capsule_cli_error("CND-CAP-007", "capsule serialization failed", presentation)
            })?;
            write_new_file(&pack.output, &bytes)
                .map_err(|message| capsule_cli_error("CND-IO-002", &message, presentation))?;
            let summary = capsule_summary(&document);
            (
                "capsule-pack",
                pack.output.display().to_string(),
                format!(
                    "packed {}: {}; program {}\n",
                    pack.output.display(),
                    summary.identity,
                    summary.program_identity
                ),
                serde_json::to_value(summary).expect("capsule summary serializes"),
            )
        }
        CapsuleOperation::Inspect(inspect) => {
            let document = read_capsule(&inspect.capsule, presentation)?;
            let summary = capsule_summary(&document);
            (
                "capsule-inspect",
                inspect.capsule.display().to_string(),
                format!(
                    "capsule {}; program {}; source {}; {} references; lock {}; presentation {}\n",
                    summary.identity,
                    summary.program_identity,
                    summary.source_revision,
                    summary.artifact_references,
                    summary.has_import_lock,
                    summary.has_presentation
                ),
                serde_json::to_value(summary).expect("capsule summary serializes"),
            )
        }
        CapsuleOperation::Check(check) => {
            let document = read_capsule(&check.capsule, presentation)?;
            let panel = parse(&document.source).map_err(|_| {
                capsule_cli_error(
                    "CND-CAP-003",
                    "validated capsule source no longer parses",
                    presentation,
                )
            })?;
            let result = CapsuleCheckResult {
                summary: capsule_summary(&document),
                panel_version: panel.version,
                definitions: panel.definitions.len(),
                root_nodes: panel.nodes.len(),
                root_cords: panel.cords.len(),
            };
            (
                "capsule-check",
                check.capsule.display().to_string(),
                format!(
                    "ok capsule {}; panel v{}; {} definitions; {} root nodes; {} root cords\n",
                    result.summary.program_identity,
                    result.panel_version,
                    result.definitions,
                    result.root_nodes,
                    result.root_cords
                ),
                serde_json::to_value(result).expect("capsule check serializes"),
            )
        }
        CapsuleOperation::Explain(explain) => {
            let document = read_capsule(&explain.capsule, presentation)?;
            let panel = parse(&document.source).map_err(|_| {
                capsule_cli_error(
                    "CND-CAP-003",
                    "validated capsule source no longer parses",
                    presentation,
                )
            })?;
            let registry = Registry::hosted_primitives();
            let resolved = registry.resolve(&panel).map_err(|error| {
                cli_error(from_resolution_error(&error), presentation, Vec::new())
            })?;
            let view = resolved.view();
            (
                "capsule-explain",
                explain.capsule.display().to_string(),
                resolved.explain(),
                serde_json::to_value(view).expect("resolved capsule view serializes"),
            )
        }
        CapsuleOperation::Unpack(unpack) => {
            let document = read_capsule(&unpack.capsule, presentation)?;
            fs::create_dir(&unpack.output_dir).map_err(|error| {
                capsule_cli_error(
                    "CND-IO-002",
                    &format!("cannot create capsule output directory: {error}"),
                    presentation,
                )
            })?;
            write_new_file(
                &unpack.output_dir.join("main.panel"),
                document.source.as_bytes(),
            )
            .map_err(|message| capsule_cli_error("CND-IO-002", &message, presentation))?;
            if let Some(lock) = &document.import_lock {
                write_new_file(
                    &unpack.output_dir.join("contract-package-lock.json"),
                    lock.text.as_bytes(),
                )
                .map_err(|message| capsule_cli_error("CND-IO-002", &message, presentation))?;
            }
            if let Some(workspace) = &document.presentation {
                write_new_file(
                    &unpack.output_dir.join("presentation.json"),
                    workspace.text.as_bytes(),
                )
                .map_err(|message| capsule_cli_error("CND-IO-002", &message, presentation))?;
            }
            write_new_file(
                &unpack.output_dir.join("capsule.json"),
                &serde_json::to_vec_pretty(&document).expect("validated capsule serializes"),
            )
            .map_err(|message| capsule_cli_error("CND-IO-002", &message, presentation))?;
            let summary = capsule_summary(&document);
            (
                "capsule-unpack",
                unpack.output_dir.display().to_string(),
                format!(
                    "unpacked {} to {} without fetching or executing artifacts\n",
                    summary.identity,
                    unpack.output_dir.display()
                ),
                serde_json::to_value(summary).expect("capsule summary serializes"),
            )
        }
        CapsuleOperation::Diff(diff) => {
            let left = read_capsule(&diff.left, presentation)?;
            let right = read_capsule(&diff.right, presentation)?;
            let result = CapsuleDiffResult {
                same_capsule: left.identity == right.identity,
                same_program: left.program_identity == right.program_identity,
                same_source_revision: left.source_revision == right.source_revision,
                same_source_semantics: left.source_semantic_identity
                    == right.source_semantic_identity,
                same_import_lock: left.import_lock == right.import_lock,
                same_artifact_references: left.artifact_references == right.artifact_references,
                same_presentation: left.presentation == right.presentation,
            };
            (
                "capsule-diff",
                format!("{}..{}", diff.left.display(), diff.right.display()),
                format!(
                    "capsule={} program={} source-revision={} source-semantics={} lock={} references={} presentation={}\n",
                    result.same_capsule,
                    result.same_program,
                    result.same_source_revision,
                    result.same_source_semantics,
                    result.same_import_lock,
                    result.same_artifact_references,
                    result.same_presentation
                ),
                serde_json::to_value(result).expect("capsule diff serializes"),
            )
        }
    };
    emit_status(status_enabled, "Capsule", &subject);
    let completion = match arguments.format {
        OutputFormat::Human => write_primary(human.as_bytes(), presentation)?,
        OutputFormat::Json => write_json_primary(
            &FiniteResult {
                schema: "conduit.result",
                schema_version: 0,
                operation: operation_name,
                result,
            },
            presentation,
        )?,
        OutputFormat::Ndjson => unreachable!("validated above"),
    };
    if completion == Completion::BrokenPipe {
        return Ok(completion);
    }
    emit_finished(status_enabled, operation_name, started.elapsed(), &subject);
    Ok(Completion::Success)
}

fn read_inline_document(
    path: &std::path::Path,
    media_type: &str,
    presentation: PresentationOptions,
) -> Result<InlineDocument, CliError> {
    let bytes = read_bounded(path, MAXIMUM_AUXILIARY_BYTES as u64)
        .map_err(|error| package_input_error(error, presentation))?;
    let text = String::from_utf8(bytes).map_err(|_| {
        capsule_cli_error(
            "CND-CAP-004",
            "capsule auxiliary document is not UTF-8",
            presentation,
        )
    })?;
    Ok(InlineDocument::new(media_type, text, "public"))
}

fn read_capsule(
    path: &std::path::Path,
    presentation: PresentationOptions,
) -> Result<CapsuleDocument, CliError> {
    let bytes = read_bounded(path, MAXIMUM_CAPSULE_DOCUMENT_BYTES)
        .map_err(|error| package_input_error(error, presentation))?;
    let document: CapsuleDocument = serde_json::from_slice(&bytes).map_err(|_| {
        capsule_cli_error(
            "CND-CAP-007",
            "capsule is not valid current JSON",
            presentation,
        )
    })?;
    document
        .validate()
        .map_err(|error| capsule_library_error(error, presentation))?;
    Ok(document)
}

fn capsule_summary(document: &CapsuleDocument) -> CapsuleSummary {
    CapsuleSummary {
        identity: document.identity.clone(),
        program_identity: document.program_identity.clone(),
        source_revision: document.source_revision.clone(),
        source_semantic_identity: document.source_semantic_identity.clone(),
        artifact_references: document.artifact_references.len(),
        has_import_lock: document.import_lock.is_some(),
        has_presentation: document.presentation.is_some(),
    }
}

fn capsule_library_error(
    error: conduit_capsule::CapsuleError,
    presentation: PresentationOptions,
) -> CliError {
    capsule_cli_error(error.code(), &error.to_string(), presentation)
}

fn capsule_cli_error(
    code: &'static str,
    message: &str,
    presentation: PresentationOptions,
) -> CliError {
    cli_error(simple_diagnostic(code, message), presentation, vec![])
}

fn package_library_error(
    error: conduit_package::PackageError,
    presentation: PresentationOptions,
) -> CliError {
    package_error(error.code(), &error.to_string(), presentation)
}

fn package_input_error(
    error: conduit_inspect::InspectionError,
    presentation: PresentationOptions,
) -> CliError {
    if error.code == "CND-INSP-005" {
        package_error(
            conduit_package::PackageReason::LimitExceeded.code(),
            "package or package metadata input limit exceeded",
            presentation,
        )
    } else {
        inspection_error(error, presentation)
    }
}

fn package_error(code: &'static str, message: &str, presentation: PresentationOptions) -> CliError {
    cli_error(simple_diagnostic(code, message), presentation, vec![])
}

fn write_new_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("refusing to overwrite {}", path.display()));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("output path {} has no file name", path.display()))?
        .to_string_lossy();
    let mut last_error = None;
    for attempt in 0..16_u8 {
        let temporary = parent.join(format!(".{file_name}.tmp-{}-{attempt}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(mut file) => {
                let result = file
                    .write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .and_then(|()| fs::hard_link(&temporary, path));
                let _ = fs::remove_file(&temporary);
                return result
                    .map_err(|error| format!("cannot create {}: {error}", path.display()));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                last_error = Some(error);
            }
            Err(error) => {
                return Err(format!("cannot create {}: {error}", path.display()));
            }
        }
    }
    Err(format!(
        "cannot create temporary package beside {}: {}",
        path.display(),
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "temporary name exhaustion".to_owned())
    ))
}

fn inspection_error(
    error: conduit_inspect::InspectionError,
    presentation: PresentationOptions,
) -> CliError {
    cli_error(
        simple_diagnostic(error.code, &error.message),
        presentation,
        vec![],
    )
}

fn compile_source_read_error(
    error: conduit_inspect::InspectionError,
    presentation: PresentationOptions,
) -> CliError {
    let code = if error.code == "CND-INSP-005" {
        "CND-CMP-009"
    } else {
        error.code
    };
    let message = if code == "CND-CMP-009" {
        "entry source byte limit exceeded"
    } else {
        &error.message
    };
    cli_error(simple_diagnostic(code, message), presentation, vec![])
}

const fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Check => "check",
        Mode::Explain => "explain",
        Mode::Run => "run",
    }
}

const fn output_format_name(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Human => "human",
        OutputFormat::Json => "json",
        OutputFormat::Ndjson => "ndjson",
    }
}

fn cli_error(
    diagnostic: OwnedDiagnostic,
    presentation: PresentationOptions,
    sources: Vec<DiagnosticSource>,
) -> CliError {
    CliError {
        diagnostic: Box::new(diagnostic),
        sources,
        presentation,
    }
}

fn output_error(error: io::Error, presentation: PresentationOptions) -> CliError {
    output_failure(&error.to_string(), presentation)
}

fn output_failure(message: &str, presentation: PresentationOptions) -> CliError {
    cli_error(
        simple_diagnostic("CND-IO-002", &format!("cannot write stdout: {message}")),
        presentation,
        vec![],
    )
}

fn simple_diagnostic(code: &str, message: &str) -> OwnedDiagnostic {
    OwnedDiagnostic {
        schema_version: DIAGNOSTIC_SCHEMA_VERSION,
        code: code.to_owned(),
        severity: conduit_diagnostics::OwnedDiagnosticSeverity::Error,
        message: message.to_owned(),
        primary: None,
        related: Vec::new(),
        arguments: Vec::new(),
        notes: Vec::new(),
        help: None,
        fixes: Vec::new(),
        semantic_path: None,
        causes: Vec::new(),
    }
}

fn emit_diagnostic(error: &CliError, environment: &EnvironmentPolicy, stderr_is_terminal: bool) {
    let rendered = match error.presentation.diagnostic_format {
        DiagnosticFormat::Json => error.diagnostic.to_json().unwrap_or_else(|failure| {
            let fallback = simple_diagnostic(
                "CND-CLI-002",
                &format!("diagnostic serialization failed: {failure}"),
            );
            fallback
                .to_json()
                .expect("the fixed fallback diagnostic is valid")
        }),
        DiagnosticFormat::Human => render_terminal(
            &error.diagnostic,
            &error.sources,
            if environment.color_enabled(error.presentation.color, stderr_is_terminal) {
                TerminalColor::Always
            } else {
                TerminalColor::Never
            },
            if error.presentation.verbose_diagnostics {
                TerminalVerbosity::Verbose
            } else {
                TerminalVerbosity::Concise
            },
        ),
    };
    let _ = writeln!(io::stderr().lock(), "{}", rendered.trim_end());
}

fn emit_status(enabled: bool, verb: &str, detail: &str) {
    if enabled {
        let _ = write!(io::stderr().lock(), "{}", status_line(verb, detail));
    }
}

fn emit_finished(enabled: bool, operation: &str, elapsed: Duration, detail: &str) {
    if enabled {
        let millis = elapsed.as_millis();
        emit_status(
            true,
            "Finished",
            &format!("{operation} in {millis} ms ({detail})"),
        );
    }
}

fn status_line(verb: &str, detail: &str) -> String {
    format!("{verb:>12} {detail}\n")
}

fn read_source(reader: &mut dyn Read, label: &str) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take((MAXIMUM_PANEL_SOURCE_BYTES as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    if bytes.len() > MAXIMUM_PANEL_SOURCE_BYTES {
        return Err(format!(
            "cannot read {label}: panel source byte limit exceeded"
        ));
    }
    String::from_utf8(bytes).map_err(|_| format!("cannot read {label}: source is not valid UTF-8"))
}

fn write_primary(bytes: &[u8], presentation: PresentationOptions) -> Result<Completion, CliError> {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => Ok(Completion::Success),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(Completion::BrokenPipe),
        Err(error) => Err(output_error(error, presentation)),
    }
}

fn write_json_primary(
    value: &impl Serialize,
    presentation: PresentationOptions,
) -> Result<Completion, CliError> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| {
        cli_error(
            simple_diagnostic(
                "CND-CLI-002",
                &format!("result serialization failed: {error}"),
            ),
            presentation,
            vec![],
        )
    })?;
    bytes.push(b'\n');
    write_primary(&bytes, presentation)
}

struct ObservedWriter<W> {
    inner: W,
    broken_pipe: bool,
    failure: Option<String>,
}

struct RunNdjsonChannelWriter<'a, W> {
    stream: &'a RefCell<RunNdjsonState<W>>,
    channel: &'static str,
}

impl<'a, W> RunNdjsonChannelWriter<'a, W> {
    const fn new(stream: &'a RefCell<RunNdjsonState<W>>, channel: &'static str) -> Self {
        Self { stream, channel }
    }
}

impl<W: Write> Write for RunNdjsonChannelWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }

        let mut emitted = 0;
        for chunk in bytes.chunks(RUN_CHANNEL_CHUNK_MAX_BYTES) {
            if let Err(error) = self
                .stream
                .borrow_mut()
                .write_channel_chunk(self.channel, chunk)
            {
                return if emitted == 0 {
                    Err(error)
                } else {
                    Ok(emitted)
                };
            }
            emitted = emitted
                .checked_add(chunk.len())
                .ok_or_else(|| io::Error::other("run-stream-write-count-overflow"))?;
        }
        Ok(emitted)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.borrow_mut().inner.flush()
    }
}

impl<W> ObservedWriter<W> {
    const fn new(inner: W) -> Self {
        Self {
            inner,
            broken_pipe: false,
            failure: None,
        }
    }
}

impl<W: Write> Write for ObservedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.inner.write(bytes) {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                self.broken_pipe = true;
                Err(error)
            }
            Err(error) => {
                self.failure = Some(error.to_string());
                Err(error)
            }
            result => result,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.flush() {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                self.broken_pipe = true;
                Err(error)
            }
            Err(error) => {
                self.failure = Some(error.to_string());
                Err(error)
            }
            result => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduct::run_stream::RUN_CHANNEL_RECORD_MAX_BYTES;
    use std::path::PathBuf;

    fn parse(values: &[&str]) -> Result<ParsedCommand, CliError> {
        parse_arguments(
            values.iter().map(OsString::from).collect(),
            PresentationOptions::default(),
        )
    }

    #[test]
    fn run_is_the_default() {
        let ParsedCommand::Execute(arguments) = parse(&["example.panel"]).unwrap() else {
            panic!("ordinary source executes");
        };
        assert_eq!(arguments.mode(), Mode::Run);
        assert_eq!(arguments.panel, Some(PathBuf::from("example.panel")));
    }

    #[test]
    fn no_path_means_panel_source_from_stdin() {
        let ParsedCommand::Execute(arguments) = parse(&[]).unwrap() else {
            panic!("empty command executes stdin");
        };
        assert_eq!(arguments.mode(), Mode::Run);
        assert_eq!(arguments.panel, None);
    }

    #[test]
    fn modes_are_exclusive_and_structured() {
        let error = parse(&["--check", "--run"]).unwrap_err();
        assert_eq!(error.diagnostic.code, "CND-CLI-001");
        assert!(error.diagnostic.message.contains("cannot be used"));
    }

    #[test]
    fn diagnostic_flags_survive_a_command_parse_failure() {
        let values = [
            OsString::from("--diagnostic-format=json"),
            OsString::from("--color"),
            OsString::from("always"),
            OsString::from("--verbose-diagnostics"),
            OsString::from("--unknown"),
        ];
        assert_eq!(
            PresentationOptions::scan(&values),
            PresentationOptions {
                diagnostic_format: DiagnosticFormat::Json,
                color: ColorChoice::Always,
                verbose_diagnostics: true,
            }
        );
    }

    #[test]
    fn color_precedence_and_non_tty_policy_are_explicit() {
        let plain = EnvironmentPolicy::default();
        assert!(!plain.color_enabled(ColorChoice::Auto, false));
        assert!(plain.color_enabled(ColorChoice::Auto, true));
        assert!(plain.color_enabled(ColorChoice::Always, false));
        assert!(!plain.color_enabled(ColorChoice::Never, true));

        let no_color = EnvironmentPolicy {
            no_color: true,
            ..plain.clone()
        };
        assert!(!no_color.color_enabled(ColorChoice::Auto, true));
        assert!(no_color.color_enabled(ColorChoice::Always, true));

        let forced = EnvironmentPolicy {
            clicolor_force: Some(OsString::from("1")),
            ..plain.clone()
        };
        assert!(forced.color_enabled(ColorChoice::Auto, false));

        let dumb = EnvironmentPolicy {
            term_dumb: true,
            ..plain
        };
        assert!(!dumb.color_enabled(ColorChoice::Auto, true));
        assert!(!dumb.status_enabled(DiagnosticFormat::Human, true));
    }

    #[test]
    fn status_is_plain_bounded_and_suppressed_for_machine_diagnostics() {
        let policy = EnvironmentPolicy::default();
        assert!(!policy.status_enabled(DiagnosticFormat::Human, false));
        assert!(!policy.status_enabled(DiagnosticFormat::Json, true));
        assert_eq!(
            status_line("Running", "fixture.panel"),
            "     Running fixture.panel\n"
        );
        assert!(!status_line("Finished", "run").contains('\u{1b}'));
    }

    #[test]
    fn every_presentation_conformance_vector_is_enforced() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/c3/conduct-cli.json")).unwrap();

        for case in fixture["presentation_cases"].as_array().unwrap() {
            let case_id = case["id"].as_str().unwrap();
            let terminal = case["terminal"].as_bool().unwrap();
            let diagnostic_format = match case["diagnostic_format"].as_str().unwrap() {
                "human" => DiagnosticFormat::Human,
                "json" => DiagnosticFormat::Json,
                value => panic!("{case_id}: unknown diagnostic format {value}"),
            };
            let color = match case["color"].as_str().unwrap() {
                "auto" => ColorChoice::Auto,
                "always" => ColorChoice::Always,
                "never" => ColorChoice::Never,
                value => panic!("{case_id}: unknown color choice {value}"),
            };
            let environment = case["environment"].as_object().unwrap();
            let policy = EnvironmentPolicy {
                no_color: environment.contains_key("NO_COLOR"),
                clicolor: environment
                    .get("CLICOLOR")
                    .and_then(serde_json::Value::as_str)
                    .map(OsString::from),
                clicolor_force: environment
                    .get("CLICOLOR_FORCE")
                    .and_then(serde_json::Value::as_str)
                    .map(OsString::from),
                term_dumb: environment.get("TERM").and_then(serde_json::Value::as_str)
                    == Some("dumb"),
            };
            let expected = &case["expected"];
            let ansi = diagnostic_format == DiagnosticFormat::Human
                && policy.color_enabled(color, terminal);
            if let Some(expected_ansi) = expected.get("ansi").and_then(serde_json::Value::as_bool) {
                assert_eq!(ansi, expected_ansi, "{case_id}: ANSI");
            }
            assert_eq!(
                policy.status_enabled(diagnostic_format, terminal),
                expected["status"].as_bool().unwrap(),
                "{case_id}: status"
            );
            assert!(
                !expected["cursor_control"].as_bool().unwrap(),
                "{case_id}: current presentation never emits cursor control"
            );
            if let Some(spinner) = expected.get("spinner") {
                assert!(
                    !spinner.as_bool().unwrap(),
                    "{case_id}: current presentation has no spinner"
                );
            }
        }
    }

    fn records(bytes: &[u8]) -> Vec<serde_json::Value> {
        bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect()
    }

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(pair, 16).unwrap()
            })
            .collect()
    }

    fn reconstructed_channel(records: &[serde_json::Value], channel: &str) -> Vec<u8> {
        records
            .iter()
            .filter(|record| record["channel"] == channel)
            .flat_map(|record| decode_hex(record["payload_hex"].as_str().unwrap()))
            .collect()
    }

    #[test]
    fn channel_writes_are_bounded_nonsemantic_chunks() {
        let stream = RefCell::new(RunNdjsonState::new(Vec::new()));
        let mut stdout = RunNdjsonChannelWriter::new(&stream, "stdout");

        assert_eq!(stdout.write(&[]).unwrap(), 0);
        stdout
            .write_all(&vec![0x5a; RUN_CHANNEL_CHUNK_MAX_BYTES])
            .unwrap();
        stdout
            .write_all(&vec![0xa5; RUN_CHANNEL_CHUNK_MAX_BYTES + 1])
            .unwrap();
        let very_large: Vec<u8> = (0..(RUN_CHANNEL_CHUNK_MAX_BYTES * 17 + 3))
            .map(|index| (index % 251) as u8)
            .collect();
        stdout.write_all(&very_large).unwrap();

        let encoded = stream.into_inner().inner;
        let records = records(&encoded);
        assert_eq!(records.len(), 21);
        assert!(
            encoded
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .all(|line| line.len() < RUN_CHANNEL_RECORD_MAX_BYTES)
        );
        for (sequence, record) in records.iter().enumerate() {
            assert_eq!(record["schema"], "conduit.run");
            assert_eq!(record["schema_version"], 0);
            assert_eq!(record["sequence"], sequence);
            assert_eq!(record["record"], "channel_chunk");
            assert!(record.get("value").is_none());
            assert!(record.get("event").is_none());
            assert!(record.get("node").is_none());
            assert!(record.get("port").is_none());
            assert!(
                record["payload_bytes"].as_u64().unwrap() <= RUN_CHANNEL_CHUNK_MAX_BYTES as u64
            );
        }

        let mut expected = vec![0x5a; RUN_CHANNEL_CHUNK_MAX_BYTES];
        expected.extend_from_slice(&vec![0xa5; RUN_CHANNEL_CHUNK_MAX_BYTES + 1]);
        expected.extend_from_slice(&very_large);
        assert_eq!(reconstructed_channel(&records, "stdout"), expected);
    }

    #[test]
    fn split_coalesced_and_interleaved_writes_preserve_only_channel_bytes() {
        fn encode(writes: &[&[u8]]) -> Vec<serde_json::Value> {
            let stream = RefCell::new(RunNdjsonState::new(Vec::new()));
            {
                let mut stdout = RunNdjsonChannelWriter::new(&stream, "stdout");
                for write in writes {
                    stdout.write_all(write).unwrap();
                }
            }
            records(&stream.into_inner().inner)
        }

        let bytes = b"two logical values with no framing";
        let one = encode(&[bytes]);
        let two = encode(&[&bytes[..9], &bytes[9..]]);
        let many: Vec<&[u8]> = bytes.chunks(3).collect();
        let many = encode(&many);
        for variant in [&one, &two, &many] {
            assert_eq!(reconstructed_channel(variant, "stdout"), bytes);
            assert!(
                variant
                    .iter()
                    .all(|record| record["record"] == "channel_chunk")
            );
        }

        let stream = RefCell::new(RunNdjsonState::new(Vec::new()));
        {
            let mut stdout = RunNdjsonChannelWriter::new(&stream, "stdout");
            let mut stderr = RunNdjsonChannelWriter::new(&stream, "stderr");
            stdout.write_all(b"out-1").unwrap();
            stderr.write_all(b"err-1").unwrap();
            stdout.write_all(b"out-2").unwrap();
            stderr.write_all(b"err-2").unwrap();
        }
        let interleaved = records(&stream.into_inner().inner);
        assert_eq!(
            interleaved
                .iter()
                .map(|record| record["sequence"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert_eq!(reconstructed_channel(&interleaved, "stdout"), b"out-1out-2");
        assert_eq!(reconstructed_channel(&interleaved, "stderr"), b"err-1err-2");
    }

    struct FailOnWrite {
        successful_writes: usize,
        writes: usize,
        kind: io::ErrorKind,
        bytes: Vec<u8>,
    }

    impl Write for FailOnWrite {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.writes >= self.successful_writes {
                return Err(io::Error::new(self.kind, "fixture-output-failure"));
            }
            self.writes += 1;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn first_and_later_chunk_failures_follow_write_partiality() {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::Other] {
            let first = RefCell::new(RunNdjsonState::new(FailOnWrite {
                successful_writes: 0,
                writes: 0,
                kind,
                bytes: Vec::new(),
            }));
            let error = RunNdjsonChannelWriter::new(&first, "stdout")
                .write(&vec![0; RUN_CHANNEL_CHUNK_MAX_BYTES + 1])
                .unwrap_err();
            assert_eq!(error.kind(), kind);
            assert!(first.into_inner().inner.bytes.is_empty());

            let later = RefCell::new(RunNdjsonState::new(FailOnWrite {
                successful_writes: 1,
                writes: 0,
                kind,
                bytes: Vec::new(),
            }));
            let written = RunNdjsonChannelWriter::new(&later, "stdout")
                .write(&vec![0; RUN_CHANNEL_CHUNK_MAX_BYTES + 1])
                .unwrap();
            assert_eq!(written, RUN_CHANNEL_CHUNK_MAX_BYTES);
            let state = later.into_inner();
            assert_eq!(records(&state.inner.bytes).len(), 1);
        }
    }

    #[test]
    fn ordinary_source_reader_stops_at_the_shared_panel_ceiling() {
        let mut exact = io::Cursor::new(vec![b'#'; MAXIMUM_PANEL_SOURCE_BYTES]);
        assert_eq!(
            read_source(&mut exact, "fixture").unwrap().len(),
            MAXIMUM_PANEL_SOURCE_BYTES
        );

        let mut oversized = io::Cursor::new(vec![b'#'; MAXIMUM_PANEL_SOURCE_BYTES + 1]);
        let error = read_source(&mut oversized, "fixture").unwrap_err();
        assert!(error.contains("panel source byte limit exceeded"));
    }
}
