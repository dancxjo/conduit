use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap_complete::generate;
use clap_complete::shells::{Bash, Elvish, Fish, PowerShell, Zsh};
use conduct::command;

fn main() -> ExitCode {
    let check = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [argument] if argument == "--check" => true,
        _ => {
            eprintln!("usage: generate-conduct-assets [--check]");
            return ExitCode::from(2);
        }
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = generated_assets();
    let mut drift = Vec::new();
    for (relative, expected) in assets {
        let path = root.join(&relative);
        if check {
            if fs::read(&path).ok().as_deref() != Some(expected.as_slice()) {
                drift.push(relative);
            }
        } else if let Err(error) = write_asset(&path, &expected) {
            eprintln!("cannot write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    }
    if drift.is_empty() {
        ExitCode::SUCCESS
    } else {
        for path in drift {
            eprintln!("generated CLI asset drift: {}", path.display());
        }
        eprintln!("run `just cli-assets` and commit the result");
        ExitCode::FAILURE
    }
}

fn generated_assets() -> Vec<(PathBuf, Vec<u8>)> {
    let mut assets = Vec::new();
    for (path, bytes) in [
        ("generated/completions/conduct.bash", completion(Bash)),
        ("generated/completions/_conduct", completion(Zsh)),
        ("generated/completions/conduct.fish", completion(Fish)),
        ("generated/completions/conduct.ps1", completion(PowerShell)),
        ("generated/completions/conduct.elv", completion(Elvish)),
    ] {
        assets.push((PathBuf::from(path), bytes));
    }

    let mut manual = Vec::new();
    clap_mangen::Man::new(command())
        .render(&mut manual)
        .expect("rendering a command model into memory cannot fail");
    manual = trim_trailing_whitespace(&manual);
    manual.extend_from_slice(
        br#".SH STREAMS
Primary human values, finite JSON results, and streaming NDJSON run records are written to stdout.
Diagnostics and terminal status are written to stderr.
.SH MACHINE OUTPUT
\fB\-\-format=json\fR selects a finite conduit.result/v1 record for check or explain.
\fB\-\-format=ndjson\fR selects ordered conduit.run/v1 records for run.
\fB\-\-diagnostic\-format=json\fR independently selects one structured diagnostic on stderr.
.SH EXIT STATUS
Zero indicates success, help, version, or normal downstream pipe closure.
Two indicates a command, source, resolution, runtime, or non-broken output failure.
"#,
    );
    assets.push((PathBuf::from("generated/man/conduct.1"), manual));
    assets
}

fn trim_trailing_whitespace(bytes: &[u8]) -> Vec<u8> {
    let text = std::str::from_utf8(bytes).expect("clap_mangen emits UTF-8 roff");
    let mut normalized = text
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    if text.ends_with('\n') {
        normalized.push(b'\n');
    }
    normalized
}

fn completion<G: clap_complete::Generator>(generator: G) -> Vec<u8> {
    let mut bytes = Vec::new();
    generate(generator, &mut command(), "conduct", &mut bytes);
    bytes
}

fn write_asset(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}
