use crate::cli::GlobalOpts;
use serde::Serialize;
use std::fmt;
use std::io::{self, Write};

pub const MAXIMUM_OUTPUT_ITEMS: usize = 64;
pub const MAXIMUM_ERROR_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Quiet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepositoryOutput {
    mode: OutputMode,
    dry_run: bool,
}

impl RepositoryOutput {
    pub const fn new(mode: OutputMode, dry_run: bool) -> Self {
        Self { mode, dry_run }
    }

    pub fn from_opts(opts: &GlobalOpts) -> Self {
        let mode = if opts.json {
            OutputMode::Json
        } else if opts.quiet {
            OutputMode::Quiet
        } else {
            OutputMode::Human
        };
        Self::new(mode, opts.dry_run)
    }

    pub const fn mode(self) -> OutputMode {
        self.mode
    }

    pub const fn dry_run(self) -> bool {
        self.dry_run
    }

    pub fn emit_json<T: Serialize>(self, value: &T) -> Result<(), OutputError> {
        if self.mode == OutputMode::Json {
            let stdout = io::stdout();
            let mut output = stdout.lock();
            serde_json::to_writer(&mut output, value).map_err(OutputError::Serialize)?;
            writeln!(output).map_err(OutputError::Write)?;
        }
        Ok(())
    }

    pub fn emit_human(
        self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<(), OutputError> {
        if self.mode == OutputMode::Human {
            let stdout = io::stdout();
            let mut output = stdout.lock();
            write(&mut output).map_err(OutputError::Write)?;
        }
        Ok(())
    }

    pub fn refusal(
        self,
        command: &'static str,
        capability: &'static str,
        reason: &'static str,
    ) -> Result<(), OutputError> {
        if reason.len() > MAXIMUM_ERROR_BYTES {
            return Err(OutputError::RefusalCapacityExceeded);
        }
        let refusal = OutputRefusal {
            schema: "conduit.tools/xtask/output-refusal@1",
            command,
            capability,
            disposition: "unsupported-before-dispatch",
            dry_run: self.dry_run,
            reason,
        };
        self.emit_json(&refusal)?;
        Err(OutputError::Unsupported(refusal))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OutputRefusal {
    pub schema: &'static str,
    pub command: &'static str,
    pub capability: &'static str,
    pub disposition: &'static str,
    pub dry_run: bool,
    pub reason: &'static str,
}

#[derive(Debug)]
pub enum OutputError {
    Serialize(serde_json::Error),
    Write(io::Error),
    Unsupported(OutputRefusal),
    RefusalCapacityExceeded,
}

impl fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(error) => {
                write!(formatter, "serialize bounded command output: {error}")
            }
            Self::Write(error) => write!(formatter, "write command output: {error}"),
            Self::Unsupported(refusal) => write!(
                formatter,
                "{} does not support {}: {}",
                refusal.command, refusal.capability, refusal.reason
            ),
            Self::RefusalCapacityExceeded => {
                formatter.write_str("command refusal exceeded its admitted output bound")
            }
        }
    }
}

impl std::error::Error for OutputError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_modes_have_one_precedence_and_retain_dry_run() {
        let opts = GlobalOpts {
            json: true,
            quiet: true,
            dry_run: true,
            locked: false,
        };
        let output = RepositoryOutput::from_opts(&opts);
        assert_eq!(output.mode(), OutputMode::Json);
        assert!(output.dry_run());
    }

    #[test]
    fn refusal_is_stable_bounded_data() {
        let error = RepositoryOutput::new(OutputMode::Quiet, false)
            .refusal(
                "pico flash",
                "quiet",
                "physical progress must remain visible",
            )
            .unwrap_err();
        let OutputError::Unsupported(refusal) = error else {
            panic!("expected refusal")
        };
        let encoded = serde_json::to_vec(&refusal).unwrap();
        assert!(encoded.len() < MAXIMUM_ERROR_BYTES);
        assert_eq!(refusal.schema, "conduit.tools/xtask/output-refusal@1");
    }
}
