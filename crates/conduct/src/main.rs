use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use conduit_diagnostics::{
    DIAGNOSTIC_SCHEMA_VERSION, DiagnosticSource, OwnedDiagnostic, TerminalColor, TerminalVerbosity,
    from_parse_error, from_resolution_error, from_runtime_error, render_terminal,
};
use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

const USAGE: &str = "\
Conduct a typed node arrangement.

Usage:
  conduct [--check | --explain | --run] [PANEL | -]

Modes:
  --check     Parse, resolve, and validate without starting nodes
  --explain   Show exact node, port, cord, type, and flow resolution
  --run       Run the panel (default)

Input:
  PANEL       Read editable source from a .panel file
  -           Read editable source from stdin (also the default with no path)

Options:
  --diagnostic-format=human|json  Select human or lossless JSON errors
  --color=auto|always|never      Select terminal styling
  --verbose-diagnostics          Include related spans, notes, paths, and causes
  -h, --help     Show this help
  -V, --version  Show the version
";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Explain,
    Run,
}

#[derive(Debug, Eq, PartialEq)]
struct Arguments {
    mode: Mode,
    panel: Option<PathBuf>,
    diagnostic_format: DiagnosticFormat,
    color: ColorChoice,
    verbose_diagnostics: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

struct CliError {
    diagnostic: Box<OwnedDiagnostic>,
    sources: Vec<DiagnosticSource>,
    format: DiagnosticFormat,
    color: ColorChoice,
    verbose: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let rendered = match error.format {
                DiagnosticFormat::Json => error.diagnostic.to_json().unwrap_or_else(|failure| {
                    format!("diagnostic serialization failed: {failure}")
                }),
                DiagnosticFormat::Human => render_terminal(
                    &error.diagnostic,
                    &error.sources,
                    match error.color {
                        ColorChoice::Always => TerminalColor::Always,
                        ColorChoice::Never => TerminalColor::Never,
                        ColorChoice::Auto if io::stderr().is_terminal() => TerminalColor::Always,
                        ColorChoice::Auto => TerminalColor::Never,
                    },
                    if error.verbose {
                        TerminalVerbosity::Verbose
                    } else {
                        TerminalVerbosity::Concise
                    },
                ),
            };
            let _ = writeln!(io::stderr().lock(), "{}", rendered.trim_end());
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), CliError> {
    let Some(arguments) = parse_arguments(env::args().skip(1)).map_err(argument_error)? else {
        return Ok(());
    };

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let document_id = arguments
        .panel
        .as_deref()
        .filter(|path| path.as_os_str() != "-")
        .map_or_else(|| "stdin".to_owned(), |path| path.display().to_string());
    let source = match arguments.panel.as_deref() {
        None => read_source(&mut stdin, "stdin").map_err(|message| {
            cli_error(
                simple_diagnostic("CND-SRC-001", &message),
                &arguments,
                vec![],
            )
        })?,
        Some(path) if path.as_os_str() == "-" => {
            read_source(&mut stdin, "stdin").map_err(|message| {
                cli_error(
                    simple_diagnostic("CND-SRC-001", &message),
                    &arguments,
                    vec![],
                )
            })?
        }
        Some(path) => fs::read_to_string(path).map_err(|error| {
            cli_error(
                simple_diagnostic(
                    "CND-SRC-001",
                    &format!("cannot read {}: {error}", path.display()),
                ),
                &arguments,
                vec![],
            )
        })?,
    };
    let source_document = DiagnosticSource::new(document_id, source.as_bytes());
    let panel = parse(&source).map_err(|error| {
        cli_error(
            from_parse_error(&error, &source_document),
            &arguments,
            vec![source_document.clone()],
        )
    })?;
    let registry = Registry::default();
    let resolved = registry.resolve(&panel).map_err(|error| {
        cli_error(
            from_resolution_error(&error),
            &arguments,
            vec![source_document.clone()],
        )
    })?;

    match arguments.mode {
        Mode::Check => {
            println!(
                "ok: panel v{}; {} definitions; {} root nodes; {} root cords",
                panel.version,
                panel.definitions.len(),
                panel.nodes.len(),
                panel.cords.len()
            );
        }
        Mode::Explain => print!("{}", resolved.explain()),
        Mode::Run => {
            let stdout = io::stdout();
            let stderr = io::stderr();
            let mut stdout = stdout.lock();
            let mut stderr = stderr.lock();
            resolved
                .run(&mut RunIo {
                    input: &mut stdin,
                    output: &mut stdout,
                    error: &mut stderr,
                })
                .map_err(|error| {
                    cli_error(
                        from_runtime_error(&error),
                        &arguments,
                        vec![source_document],
                    )
                })?;
        }
    }
    Ok(())
}

fn cli_error(
    diagnostic: OwnedDiagnostic,
    arguments: &Arguments,
    sources: Vec<DiagnosticSource>,
) -> CliError {
    CliError {
        diagnostic: Box::new(diagnostic),
        sources,
        format: arguments.diagnostic_format,
        color: arguments.color,
        verbose: arguments.verbose_diagnostics,
    }
}

fn argument_error(message: String) -> CliError {
    CliError {
        diagnostic: Box::new(simple_diagnostic("CND-SRC-001", &message)),
        sources: Vec::new(),
        format: DiagnosticFormat::Human,
        color: ColorChoice::Auto,
        verbose: false,
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

fn read_source(reader: &mut dyn Read, label: &str) -> Result<String, String> {
    let mut source = String::new();
    reader
        .read_to_string(&mut source)
        .map_err(|error| format!("CND-SRC-001: cannot read {label}: {error}"))?;
    Ok(source)
}

fn parse_arguments(
    arguments: impl IntoIterator<Item = String>,
) -> Result<Option<Arguments>, String> {
    let mut mode = None;
    let mut panel = None;
    let mut diagnostic_format = DiagnosticFormat::Human;
    let mut color = ColorChoice::Auto;
    let mut verbose_diagnostics = false;
    for argument in arguments {
        match argument.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("conduct {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--check" => set_mode(&mut mode, Mode::Check)?,
            "--explain" => set_mode(&mut mode, Mode::Explain)?,
            "--run" => set_mode(&mut mode, Mode::Run)?,
            "--diagnostic-format=human" => diagnostic_format = DiagnosticFormat::Human,
            "--diagnostic-format=json" => diagnostic_format = DiagnosticFormat::Json,
            "--color=auto" => color = ColorChoice::Auto,
            "--color=always" => color = ColorChoice::Always,
            "--color=never" => color = ColorChoice::Never,
            "--verbose-diagnostics" => verbose_diagnostics = true,
            value if value.starts_with('-') && value != "-" => {
                return Err(format!("unknown option `{value}`\n\n{USAGE}"));
            }
            value => {
                if panel.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("only one PANEL may be supplied\n\n{USAGE}"));
                }
            }
        }
    }
    Ok(Some(Arguments {
        mode: mode.unwrap_or(Mode::Run),
        panel,
        diagnostic_format,
        color,
        verbose_diagnostics,
    }))
}

fn set_mode(current: &mut Option<Mode>, requested: Mode) -> Result<(), String> {
    if let Some(existing) = current {
        if *existing != requested {
            return Err("--check, --explain, and --run are mutually exclusive".to_owned());
        }
    }
    *current = Some(requested);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_is_the_default() {
        assert_eq!(
            parse_arguments(["example.panel".to_owned()]).expect("valid arguments"),
            Some(Arguments {
                mode: Mode::Run,
                panel: Some(PathBuf::from("example.panel")),
                diagnostic_format: DiagnosticFormat::Human,
                color: ColorChoice::Auto,
                verbose_diagnostics: false,
            })
        );
    }

    #[test]
    fn no_path_means_panel_source_from_stdin() {
        assert_eq!(
            parse_arguments(Vec::<String>::new()).expect("valid arguments"),
            Some(Arguments {
                mode: Mode::Run,
                panel: None,
                diagnostic_format: DiagnosticFormat::Human,
                color: ColorChoice::Auto,
                verbose_diagnostics: false,
            })
        );
    }

    #[test]
    fn modes_are_exclusive() {
        assert!(
            parse_arguments(["--check".to_owned(), "--run".to_owned()])
                .expect_err("conflict")
                .contains("mutually exclusive")
        );
    }
}
