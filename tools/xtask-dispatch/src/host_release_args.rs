//! Parse the lightweight entrance with the full release platform vocabulary.
use super::host_release::{ReleaseOptions, ReleasePlatform};
use clap::ValueEnum;
use std::path::PathBuf;

pub(super) struct ReleaseArgs {
    pub output: PathBuf,
    pub platform: ReleasePlatform,
    pub source_identity: Option<String>,
    pub presentation: ReleaseOptions,
}

pub(super) fn parse(options: &[String]) -> Result<ReleaseArgs, String> {
    let mut values = options.iter();
    let mut output = None;
    let mut platform = None;
    let mut source_identity = None;
    let mut json = false;
    let mut quiet = false;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--json" => json = true,
            "--quiet" => quiet = true,
            "--output" => {
                output = Some(PathBuf::from(values.next().ok_or("missing --output path")?));
            }
            "--platform" => {
                let value = values.next().ok_or("missing --platform value")?;
                platform = Some(
                    ReleasePlatform::from_str(value, false)
                        .map_err(|_| format!("unsupported release platform: {value}"))?,
                );
            }
            "--source-identity" => {
                source_identity = Some(
                    values
                        .next()
                        .cloned()
                        .ok_or("missing --source-identity value")?,
                );
            }
            other => return Err(format!("unsupported host release argument: {other}")),
        }
    }
    Ok(ReleaseArgs {
        output: output.ok_or("host release requires --output")?,
        platform: platform.ok_or("host release requires --platform")?,
        source_identity,
        presentation: ReleaseOptions { json, quiet },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn native_linux_selection_reaches_the_shared_release_platform() {
        let parsed = parse(&arguments(&[
            "--platform",
            "linux-x86_64",
            "--output",
            "bundle",
            "--source-identity",
            "git:reviewed",
            "--locked",
            "--json",
            "--quiet",
        ]))
        .unwrap();
        assert!(matches!(parsed.platform, ReleasePlatform::LinuxX86_64));
        assert_eq!(parsed.output, PathBuf::from("bundle"));
        assert_eq!(parsed.source_identity.as_deref(), Some("git:reviewed"));
        assert!(parsed.presentation.json && parsed.presentation.quiet);
    }
    #[test]
    fn every_full_cli_platform_is_available_through_the_lightweight_parser() {
        for platform in ReleasePlatform::value_variants() {
            let value = platform.to_possible_value().unwrap();
            let parsed = parse(&arguments(&[
                "--output",
                "bundle",
                "--platform",
                value.get_name(),
            ]))
            .unwrap();
            assert_eq!(
                parsed.platform.to_possible_value().unwrap().get_name(),
                value.get_name()
            );
        }
    }
    #[test]
    fn missing_unknown_and_foreign_arguments_refuse_before_manufacture() {
        for values in [
            vec!["--output", "bundle"],
            vec!["--platform", "linux-x86_64"],
            vec!["--output", "bundle", "--platform"],
            vec!["--output", "bundle", "--platform", "invented"],
            vec![
                "--output",
                "bundle",
                "--platform",
                "linux-x86_64",
                "--run-shell",
            ],
        ] {
            assert!(parse(&arguments(&values)).is_err());
        }
    }
}
