use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

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
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let Some(arguments) = parse_arguments(env::args().skip(1))? else {
        return Ok(());
    };

    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let source = match arguments.panel.as_deref() {
        None => read_source(&mut stdin, "stdin")?,
        Some(path) if path.as_os_str() == "-" => read_source(&mut stdin, "stdin")?,
        Some(path) => fs::read_to_string(path)
            .map_err(|error| format!("CND-SRC-001: cannot read {}: {error}", path.display()))?,
    };
    let panel = parse(&source).map_err(|error| error.to_string())?;
    let registry = Registry::default();
    let resolved = registry
        .resolve(&panel)
        .map_err(|error| error.to_string())?;

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
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
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
