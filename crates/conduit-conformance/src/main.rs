use std::{
    env,
    io::{self, BufReader},
    path::{Path, PathBuf},
    process::ExitCode,
};

use conduit_conformance::{
    check_results, load_manifest, run_reference, verify_reference_fixtures, write_requests,
};

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("conformance error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "reference".to_owned());
    let manifest = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(default_manifest);
    if args.next().is_some() {
        return Err(
            "usage: conduit-conformance [audit|verify-fixtures|requests|check-results|reference] [manifest]".into(),
        );
    }
    let loaded = load_manifest(&manifest)?;
    match command.as_str() {
        "audit" => {
            println!(
                "ok {} revision {}: {} suites, {} cases",
                loaded.manifest.fixture_version,
                loaded.manifest.manifest_revision,
                loaded.manifest.suites.len(),
                loaded.cases.len()
            );
            Ok(true)
        }
        "requests" => {
            write_requests(&loaded, io::stdout().lock())?;
            Ok(true)
        }
        "verify-fixtures" => {
            verify_reference_fixtures(&loaded)?;
            println!(
                "ok reference fixtures: {} suites, {} normative cases",
                loaded.manifest.suites.len(),
                loaded.cases.len()
            );
            Ok(true)
        }
        "check-results" => check_results(
            &loaded,
            BufReader::new(io::stdin().lock()),
            io::stdout().lock(),
        )
        .map_err(Into::into),
        "reference" => {
            run_reference(&loaded)?;
            println!(
                "ok Rust reference: {} suites, {} normative cases",
                loaded.manifest.suites.len(),
                loaded.cases.len()
            );
            Ok(true)
        }
        _ => Err(format!(
            "unknown command {command:?}; expected audit, verify-fixtures, requests, check-results, or reference"
        )
        .into()),
    }
}

fn default_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("conformance/v1/manifest.json")
}
