use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::error::ErrorKind;
use clap::{ArgGroup, Parser, ValueEnum};
use conduit_diagnostics::{
    DIAGNOSTIC_SCHEMA_VERSION, DiagnosticSource, OwnedDiagnostic, TerminalColor, TerminalVerbosity,
    from_parse_error, from_resolution_error, from_runtime_error, render_terminal,
};
use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Explain,
    Run,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum DiagnosticFormat {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// Check, explain, and run one typed node arrangement.
#[derive(Debug, Eq, Parser, PartialEq)]
#[command(
    name = "conduct",
    version,
    about = "Conduct a typed node arrangement.",
    override_usage = "conduct [--check | --explain | --run] [PANEL | -]",
    disable_help_subcommand = true,
    group(ArgGroup::new("mode").args(["check", "explain", "run"]).multiple(false))
)]
struct Arguments {
    /// Parse, resolve, and validate without starting nodes.
    #[arg(long, group = "mode")]
    check: bool,

    /// Show exact node, port, cord, type, and flow resolution.
    #[arg(long, group = "mode")]
    explain: bool,

    /// Run the panel (the default mode).
    #[arg(long, group = "mode")]
    run: bool,

    /// Select human or lossless JSON diagnostics on stderr.
    #[arg(long, value_enum, default_value_t)]
    diagnostic_format: DiagnosticFormat,

    /// Select diagnostic terminal styling.
    #[arg(long, value_enum, default_value_t)]
    color: ColorChoice,

    /// Include related spans, notes, paths, and causes.
    #[arg(long)]
    verbose_diagnostics: bool,

    /// Read editable source from this file, or `-`/no path for stdin.
    #[arg(value_name = "PANEL")]
    panel: Option<PathBuf>,
}

impl Arguments {
    const fn mode(&self) -> Mode {
        if self.check {
            Mode::Check
        } else if self.explain {
            Mode::Explain
        } else {
            Mode::Run
        }
    }

    const fn presentation(&self) -> PresentationOptions {
        PresentationOptions {
            diagnostic_format: self.diagnostic_format,
            color: self.color,
            verbose_diagnostics: self.verbose_diagnostics,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PresentationOptions {
    diagnostic_format: DiagnosticFormat,
    color: ColorChoice,
    verbose_diagnostics: bool,
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
    let presentation = arguments.presentation();
    let started = Instant::now();

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let document_id = arguments
        .panel
        .as_deref()
        .filter(|path| path.as_os_str() != "-")
        .map_or_else(|| "stdin".to_owned(), |path| path.display().to_string());
    let status_enabled =
        environment.status_enabled(arguments.diagnostic_format, stderr_is_terminal);
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
        Some(path) => fs::read_to_string(path).map_err(|error| {
            cli_error(
                simple_diagnostic(
                    "CND-IO-001",
                    &format!("cannot read {}: {error}", path.display()),
                ),
                presentation,
                vec![],
            )
        })?,
    };
    let source_document = DiagnosticSource::new(document_id.clone(), source.as_bytes());
    let panel = parse(&source).map_err(|error| {
        cli_error(
            from_parse_error(&error, &source_document),
            presentation,
            vec![source_document.clone()],
        )
    })?;

    emit_status(status_enabled, "Resolving", &document_id);
    let registry = Registry::default();
    let resolved = registry.resolve(&panel).map_err(|error| {
        cli_error(
            from_resolution_error(&error),
            presentation,
            vec![source_document.clone()],
        )
    })?;

    match arguments.mode() {
        Mode::Check => {
            let output = format!(
                "ok: panel v{}; {} definitions; {} root nodes; {} root cords\n",
                panel.version,
                panel.definitions.len(),
                panel.nodes.len(),
                panel.cords.len()
            );
            let completion = write_primary(output.as_bytes(), presentation)?;
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
            let completion = write_primary(resolved.explain().as_bytes(), presentation)?;
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
            let mut output = ObservedWriter::new(stdout.lock());
            let summary = {
                let mut error = stderr.lock();
                resolved.run(&mut RunIo {
                    input: &mut stdin,
                    output: &mut output,
                    error: &mut error,
                })
            };
            if output.broken_pipe {
                return Ok(Completion::BrokenPipe);
            }
            let summary = summary.map_err(|error| {
                cli_error(
                    from_runtime_error(&error),
                    presentation,
                    vec![source_document],
                )
            })?;
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
    let mut source = String::new();
    reader
        .read_to_string(&mut source)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    Ok(source)
}

fn write_primary(bytes: &[u8], presentation: PresentationOptions) -> Result<Completion, CliError> {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => Ok(Completion::Success),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(Completion::BrokenPipe),
        Err(error) => Err(cli_error(
            simple_diagnostic("CND-IO-002", &format!("cannot write stdout: {error}")),
            presentation,
            vec![],
        )),
    }
}

struct ObservedWriter<W> {
    inner: W,
    broken_pipe: bool,
}

impl<W> ObservedWriter<W> {
    const fn new(inner: W) -> Self {
        Self {
            inner,
            broken_pipe: false,
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
            result => result,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            serde_json::from_str(include_str!("../../../conformance/c3/conduct-cli-v1.json"))
                .unwrap();

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
                "{case_id}: version 1 never emits cursor control"
            );
            if let Some(spinner) = expected.get("spinner") {
                assert!(
                    !spinner.as_bool().unwrap(),
                    "{case_id}: version 1 has no spinner"
                );
            }
        }
    }
}
