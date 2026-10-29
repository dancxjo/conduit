//! Parse and execute the dependency-light Host release entrance.
use super::host_release;
use std::path::PathBuf;

pub(super) fn run_host_release(options: &[String]) -> Result<(), String> {
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
                platform = Some(match values.next().map(String::as_str) {
                    Some("browser") => host_release::ReleasePlatform::Browser,
                    Some("linux") => host_release::ReleasePlatform::Linux,
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
    let source_identity = match source_identity {
        Some(identity) => identity,
        None => command_identity("git", &["rev-parse", "HEAD"])?,
    };
    host_release::run(
        &output,
        platform,
        &source_identity,
        &host_release::ReleaseOptions { json, quiet },
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
