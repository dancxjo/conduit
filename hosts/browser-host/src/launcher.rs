//! Supported desktop launcher boundary for a resolved browser Host URL.

use std::process::{Command, ExitStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DesktopPlatform {
    Linux,
    MacOs,
}

impl DesktopPlatform {
    fn from_os(os: &str) -> Result<Self, &'static str> {
        match os {
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::MacOs),
            _ => Err("unsupported desktop platform"),
        }
    }

    fn opener(self) -> &'static str {
        match self {
            Self::Linux => "xdg-open",
            Self::MacOs => "open",
        }
    }
}

trait CommandBoundary {
    fn invoke(&mut self, program: &str, url: &str) -> std::io::Result<ExitStatus>;
}

struct SystemCommand;

impl CommandBoundary for SystemCommand {
    fn invoke(&mut self, program: &str, url: &str) -> std::io::Result<ExitStatus> {
        Command::new(program).arg(url).status()
    }
}

pub fn open(url: &str) -> Result<(), String> {
    let platform = DesktopPlatform::from_os(std::env::consts::OS)
        .map_err(|reason| format!("cannot launch browser Host at {url}: {reason}"))?;
    open_with(platform, url, &mut SystemCommand)
}

fn open_with(
    platform: DesktopPlatform,
    url: &str,
    command: &mut impl CommandBoundary,
) -> Result<(), String> {
    let opener = platform.opener();
    let status = command
        .invoke(opener, url)
        .map_err(|error| format!("cannot launch browser Host at {url} with {opener}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cannot launch browser Host at {url} with {opener}: opener exited with {status}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[derive(Default)]
    struct RecordingCommand {
        calls: Vec<(String, String)>,
        status: i32,
        error: Option<std::io::ErrorKind>,
    }

    impl CommandBoundary for RecordingCommand {
        fn invoke(&mut self, program: &str, url: &str) -> std::io::Result<ExitStatus> {
            self.calls.push((program.into(), url.into()));
            self.error
                .map(|kind| Err(std::io::Error::from(kind)))
                .unwrap_or_else(|| Ok(ExitStatus::from_raw(self.status)))
        }
    }

    #[test]
    fn selects_only_supported_openers() {
        assert_eq!(
            DesktopPlatform::from_os("linux"),
            Ok(DesktopPlatform::Linux)
        );
        assert_eq!(
            DesktopPlatform::from_os("macos"),
            Ok(DesktopPlatform::MacOs)
        );
        assert_eq!(
            DesktopPlatform::from_os("windows"),
            Err("unsupported desktop platform")
        );
    }

    #[test]
    fn invokes_the_platform_opener_once_with_the_exact_url() {
        let url = "http://127.0.0.1:43123/";
        let mut linux = RecordingCommand::default();
        open_with(DesktopPlatform::Linux, url, &mut linux).unwrap();
        assert_eq!(linux.calls, [("xdg-open".into(), url.into())]);

        let mut macos = RecordingCommand::default();
        open_with(DesktopPlatform::MacOs, url, &mut macos).unwrap();
        assert_eq!(macos.calls, [("open".into(), url.into())]);
    }

    #[test]
    fn failures_retain_the_exact_url_and_opener() {
        let url = "http://127.0.0.1:43123/";
        let mut missing = RecordingCommand {
            error: Some(std::io::ErrorKind::NotFound),
            ..RecordingCommand::default()
        };
        let error = open_with(DesktopPlatform::Linux, url, &mut missing).unwrap_err();
        assert!(error.contains(url));
        assert!(error.contains("xdg-open"));

        let mut failed = RecordingCommand {
            status: 9 << 8,
            ..RecordingCommand::default()
        };
        let error = open_with(DesktopPlatform::MacOs, url, &mut failed).unwrap_err();
        assert!(error.contains(url));
        assert!(error.contains("exit status: 9"));
    }
}
