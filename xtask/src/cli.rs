use clap::{Args, Parser, Subcommand, ValueEnum};

/// Repository orchestration task runner for Conduit.
#[derive(Parser, Debug)]
#[command(name = "xtask", about = "Conduit repository orchestration")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOpts,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone, Default)]
pub struct GlobalOpts {
    /// Print planned probes or commands without executing them.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Suppress non-error human output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Emit one structured JSON report to stdout.
    #[arg(long, global = true)]
    pub json: bool,

    /// Internal runner policy reserved for future multi-step commands.
    #[arg(skip)]
    pub keep_going: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inspect prerequisites (smoke command; check/demo/prove added in later PRs).
    Doctor(DoctorArgs),
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// What to inspect (default: all).
    #[arg(default_value = "all")]
    pub target: DoctorTarget,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorTarget {
    All,
    Browser,
    Pico,
}

impl DoctorTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Browser => "browser",
            Self::Pico => "pico",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_accepts_global_options_before_or_after_the_subcommand() {
        let before = Cli::try_parse_from(["xtask", "--dry-run", "doctor", "pico"])
            .expect("global option before command");
        assert!(before.global.dry_run);
        assert!(matches!(
            before.command,
            Command::Doctor(DoctorArgs {
                target: DoctorTarget::Pico
            })
        ));

        let after = Cli::try_parse_from(["xtask", "doctor", "browser", "--json"])
            .expect("global option after command");
        assert!(after.global.json);
        assert!(matches!(
            after.command,
            Command::Doctor(DoctorArgs {
                target: DoctorTarget::Browser
            })
        ));
    }

    #[test]
    fn unsupported_verbose_option_is_not_advertised() {
        assert!(Cli::try_parse_from(["xtask", "--verbose", "doctor"]).is_err());
    }
}
