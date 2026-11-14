//! Exact existing-artifact review through the same official release validator.
use super::host_release;
use std::{collections::BTreeMap, path::PathBuf};

pub(crate) fn run_host_release_artifact(arguments: &[String]) -> Result<(), String> {
    let (values, dry_run) = parse(arguments)?;
    let path = |name: &str| PathBuf::from(values[name]);
    host_release::review_artifact(
        &path("--configuration"),
        &path("--image"),
        &path("--executable"),
        values["--sha256"],
        &path("--output"),
        dry_run,
    )
    .map_err(|error| error.to_string())
}
fn parse(arguments: &[String]) -> Result<(BTreeMap<&str, &str>, bool), String> {
    let mut values = BTreeMap::new();
    let mut dry_run = false;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" => dry_run = true,
            "--quiet" | "--json" | "--locked" => {}
            key @ ("--configuration" | "--image" | "--executable" | "--sha256" | "--output") => {
                let value = arguments
                    .next()
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("missing {key} value"))?;
                if values.insert(key, value.as_str()).is_some() {
                    return Err(format!("duplicate artifact review option {key}"));
                }
            }
            other => return Err(format!("unsupported artifact review argument: {other}")),
        }
    }
    for key in [
        "--configuration",
        "--image",
        "--executable",
        "--sha256",
        "--output",
    ] {
        if !values.contains_key(key) {
            return Err(format!("artifact review requires {key}"));
        }
    }
    Ok((values, dry_run))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_review_options_preserve_all_paths_pin_and_dry_run() {
        let args = [
            "--configuration",
            "host source.conduit",
            "--image",
            "image.json",
            "--executable",
            "candidate",
            "--sha256",
            "pin",
            "--output",
            "new directory",
            "--dry-run",
        ]
        .map(str::to_owned);
        let (parsed, dry) = parse(&args).unwrap();
        assert!(dry);
        assert_eq!(parsed["--sha256"], "pin");
        assert_eq!(parsed["--configuration"], "host source.conduit");
        assert_eq!(parsed["--output"], "new directory");
        let mut duplicate = args.to_vec();
        duplicate.extend(["--sha256".into(), "other".into()]);
        assert!(parse(&duplicate).is_err());
        assert!(parse(&["--output".into()]).is_err());
        assert!(parse(&["--unknown".into()]).is_err());
    }
}
