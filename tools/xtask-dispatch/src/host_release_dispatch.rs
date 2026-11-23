//! Parse and execute the dependency-light Host release entrance.
use super::host_release;
use std::path::PathBuf;

struct ParsedRelease {
    output: PathBuf,
    platform: host_release::ReleasePlatform,
    source_identity: Option<String>,
    options: host_release::ReleaseOptions,
}
fn parse_options(options: &[String]) -> Result<ParsedRelease, String> {
    let mut values = options.iter();
    let mut output = None;
    let mut platform = None;
    let mut source_identity = None;
    let mut json = false;
    let mut quiet = false;
    let mut native_display = false;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--json" => json = true,
            "--quiet" => quiet = true,
            "--native-display" => native_display = true,
            "--output" => {
                output = Some(PathBuf::from(values.next().ok_or("missing --output path")?));
            }
            "--platform" => {
                platform = Some(match values.next().map(String::as_str) {
                    Some("browser") => host_release::ReleasePlatform::Browser,
                    Some("linux") => host_release::ReleasePlatform::Linux,
                    Some("linux-x86-64") => host_release::ReleasePlatform::LinuxX86_64,
                    Some("linux-aarch64") => host_release::ReleasePlatform::LinuxAarch64,
                    Some("linux-armv6") => host_release::ReleasePlatform::LinuxArmv6,
                    Some("windows") => host_release::ReleasePlatform::Windows,
                    Some("macos") => host_release::ReleasePlatform::Macos,
                    Some(other) => return Err(format!("unsupported release platform: {other}")),
                    None => return Err("missing --platform value".into()),
                });
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
    let output = output.ok_or("host release requires --output")?;
    let platform = platform.ok_or("host release requires --platform")?;
    Ok(ParsedRelease {
        output,
        platform,
        source_identity,
        options: host_release::ReleaseOptions {
            json,
            quiet,
            native_display,
            retained_linux_executable: None,
        },
    })
}
pub(super) fn run_host_release(options: &[String]) -> Result<(), String> {
    let parsed = parse_options(options)?;
    host_release::validate_options(parsed.platform, parsed.options.native_display)?;
    let source_identity = match parsed.source_identity {
        Some(identity) => identity,
        None => command_identity("git", &["rev-parse", "HEAD"])?,
    };
    host_release::run(
        &parsed.output,
        parsed.platform,
        &source_identity,
        &parsed.options,
    )
    .map_err(|error| error.to_string())
}

fn command_identity(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot execute {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot derive exact identity from {program}: {}",
            output.status
        ));
    }
    let identity = String::from_utf8(output.stdout)
        .map_err(|error| format!("{program} identity is not UTF-8: {error}"))?
        .trim()
        .to_owned();
    if identity.is_empty() {
        return Err(format!("{program} returned an empty identity"));
    }
    Ok(identity)
}

#[cfg(test)]
#[test]
fn armv6_release_platform_is_accepted_before_required_output_validation() {
    let options = ["--platform".to_owned(), "linux-armv6".to_owned()];
    assert_eq!(
        run_host_release(&options).unwrap_err(),
        "host release requires --output"
    );
}

#[cfg(test)]
#[test]
fn aarch64_release_platform_is_accepted_before_required_output_validation() {
    let options = ["--platform".to_owned(), "linux-aarch64".to_owned()];
    assert_eq!(
        run_host_release(&options).unwrap_err(),
        "host release requires --output"
    );
}

#[cfg(test)]
#[test]
fn native_release_selection_is_parsed_without_building_or_resolving_source() {
    let values = [
        "--platform",
        "linux-x86-64",
        "--native-display",
        "--output",
        "target/fixture",
        "--source-identity",
        "fixture",
    ]
    .map(str::to_owned);
    let parsed = parse_options(&values).unwrap();
    assert_eq!(parsed.platform, host_release::ReleasePlatform::LinuxX86_64);
    assert!(parsed.options.native_display);
    assert_eq!(parsed.source_identity.as_deref(), Some("fixture"));
    let defaults = parse_options(&[
        "--platform".into(),
        "linux".into(),
        "--output".into(),
        "target/fixture".into(),
    ])
    .unwrap();
    assert!(!defaults.options.native_display);
}
