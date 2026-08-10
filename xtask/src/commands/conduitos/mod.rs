mod architecture_matrix;
mod build;
mod image;
mod profile;
mod prove;
mod report;
mod run;
mod std_gap;

use std::fmt;

use clap::{Args, Subcommand, ValueEnum};

use crate::cli::GlobalOpts;

#[derive(Args, Debug)]
pub struct ConduitosArgs {
    #[command(subcommand)]
    command: ConduitosCommand,
}

#[derive(Subcommand, Debug)]
enum ConduitosCommand {
    /// Verify and report the pinned Limine architecture/backend matrix.
    ArchitectureMatrix,
    /// Compile and mechanically inspect the freestanding executable.
    Build(TargetArgs),
    /// Create the tiny pinned-Limine hybrid ISO image.
    Image(TargetArgs),
    /// Boot one deterministic QEMU session and validate its boot Sign.
    Run(TargetArgs),
    /// Prove compile/link/image/boot truth and fresh boot identities.
    Prove(TargetArgs),
    /// Inventory the portable std nucleus and classify the exact ConduitOS gap.
    StdGap,
}

#[derive(Args, Debug, Clone, Copy)]
struct TargetArgs {
    /// Architecture backend selected explicitly from the pinned Limine matrix.
    #[arg(long, value_enum, default_value_t = ConduitosArch::X86_64)]
    arch: ConduitosArch,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitosArch {
    Ia32,
    X86_64,
    Aarch64,
    Riscv64,
    Loongarch64,
}

impl ConduitosArch {
    const ALL: [Self; 5] = [
        Self::Ia32,
        Self::X86_64,
        Self::Aarch64,
        Self::Riscv64,
        Self::Loongarch64,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ia32 => "ia32",
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::Riscv64 => "riscv64",
            Self::Loongarch64 => "loongarch64",
        }
    }

    fn require_executable_backend(self) -> Result<(), ConduitosError> {
        if self == Self::X86_64 {
            Ok(())
        } else {
            Err(ConduitosError::refusal(
                "unsupported-architecture-backend",
                format!(
                    "{} is present in the pinned Limine matrix but has no accepted ConduitOS executable backend",
                    self.as_str()
                ),
            ))
        }
    }
}

#[derive(Debug)]
pub struct ConduitosError {
    reason: &'static str,
    detail: String,
}

impl ConduitosError {
    fn refusal(reason: &'static str, detail: impl Into<String>) -> Self {
        const MAX_DETAIL_BYTES: usize = 512;
        let detail = detail.into();
        let end = detail
            .char_indices()
            .map(|(index, character)| index + character.len_utf8())
            .take_while(|end| *end <= MAX_DETAIL_BYTES)
            .last()
            .unwrap_or(0);
        Self {
            reason,
            detail: detail[..end].to_owned(),
        }
    }
}

impl fmt::Display for ConduitosError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ConduitOS proof refusal [{}]: {}",
            self.reason, self.detail
        )
    }
}

impl std::error::Error for ConduitosError {}

pub fn run(args: ConduitosArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    match args.command {
        ConduitosCommand::ArchitectureMatrix => architecture_matrix::execute(opts),
        ConduitosCommand::Build(target) => {
            target.arch.require_executable_backend()?;
            build::execute(target.arch, opts).map(|_| ())
        }
        ConduitosCommand::Image(target) => {
            target.arch.require_executable_backend()?;
            image::execute(target.arch, opts).map(|_| ())
        }
        ConduitosCommand::Run(target) => {
            target.arch.require_executable_backend()?;
            run::execute(target.arch, opts).map(|_| ())
        }
        ConduitosCommand::Prove(target) => {
            target.arch.require_executable_backend()?;
            prove::execute(target.arch, opts)
        }
        ConduitosCommand::StdGap => std_gap::execute(opts),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Command};

    use super::*;

    #[test]
    fn refusal_detail_is_bounded_on_a_utf8_boundary() {
        let error = ConduitosError::refusal("test", "é".repeat(300));

        assert_eq!(error.detail.len(), 512);
        assert!(error.detail.is_char_boundary(error.detail.len()));
    }

    #[test]
    fn every_pinned_matrix_name_is_architecture_valued() {
        for name in ["ia32", "x86-64", "aarch64", "riscv64", "loongarch64"] {
            let parsed = Cli::try_parse_from(["xtask", "conduitos", "build", "--arch", name])
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(matches!(parsed.command, Command::Conduitos(_)));
        }
    }

    #[test]
    fn unavailable_backend_refuses_instead_of_aliasing_x86_64() {
        let error = ConduitosArch::Aarch64
            .require_executable_backend()
            .unwrap_err();
        assert_eq!(error.reason, "unsupported-architecture-backend");
    }
}
