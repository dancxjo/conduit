mod aarch64_a0;
mod aarch64_a1;
mod active_rescue_proof;
mod architecture_matrix;
mod armv6_rpi_b_plus_a0;
mod armv6_rpi_b_plus_image;
mod armv6_rpi_b_plus_run;
mod armv6_rpi_board;
mod armv6_rpi_flash;
mod armv6_rpi_physical;
mod build;
mod demo;
mod front_door_proof;
mod hid_proof;
mod hid_qmp;
mod hid_run;
mod hotplug_proof;
mod hotplug_qmp;
mod ia32_a0;
mod ia32_a1;
mod ia32_a2;
mod image;
mod journey_proof;
mod keyboard_proof;
mod keyboard_run;
mod keyboard_text_run;
mod loongarch64_a0;
#[allow(dead_code)]
mod loongarch64_a1;
#[allow(dead_code)]
mod loongarch64_a2;
#[allow(dead_code)]
mod loongarch64_a3;
mod loongarch64_a4;
mod opl2_proof;
mod pc_speaker_proof;
mod product_readiness_matrix;
mod profile;
mod prove;
mod report;
mod rescue_proof;
mod riscv64_a0;
#[allow(dead_code)]
mod riscv64_a1;
#[allow(dead_code)]
mod riscv64_a2;
#[allow(dead_code)]
mod riscv64_a3;
mod riscv64_a4;
mod run;
mod std_gap;
pub(crate) mod target_backend;
pub(crate) mod target_build;
mod target_lowering;
mod timing_profile;
mod usb_proof;
mod usb_run;
mod xhci_proof;

use std::{fmt, path::PathBuf};

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
    /// Report exact earned Product Spine cells independently of A0-A4.
    ProductReadinessMatrix,
    /// Compile and mechanically inspect the freestanding executable.
    Build(TargetArgs),
    /// Create the tiny pinned-Limine hybrid ISO image.
    Image(TargetArgs),
    /// Erase, write, and byte-verify one explicitly confirmed removable device.
    Flash(FlashArgs),
    /// Capture and validate one exact physical BCM2835 Raspberry Pi UART boot.
    RpiPhysicalProof(RpiPhysicalProofArgs),
    /// Open a visible interactive QEMU session without making proof claims.
    Demo(DemoArgs),
    /// Prove the normal IMAGE zero-Body front door and long-lived interaction.
    FrontDoorProof,
    /// Prove the normal IMAGE Body/Wake/Plan/Play product journey.
    JourneyProof,
    /// Boot one deterministic QEMU session and validate its boot Sign.
    Run(TargetArgs),
    /// Prove compile/link/image/boot truth and fresh boot identities.
    Prove(ProveArgs),
    /// Inventory the portable std nucleus and classify the exact ConduitOS gap.
    StdGap,
    /// Prove one exact deterministic deadline-bounded local Plan and refusal.
    TimingProfile,
    /// Prove one real bounded xHCI Base and fail-closed controller absence.
    XhciProof,
    /// Prove one real bounded root-attached USB device without semantic input.
    UsbProof,
    /// Prove one real HID boot-keyboard press/release stream without semantics.
    HidProof,
    /// Prove the exact portable keyboard offer, Plan, Play, and event values.
    KeyboardProof,
    /// Prove the exact PC-speaker offer, Plan, production-kernel Play, and Base effects.
    PcSpeakerProof,
    /// Prove real USB keyboard detach/reattach across immutable and fresh Plans.
    HotplugProof,
    /// Prove one low-level local rescue request and real fresh boot.
    RescueProof,
    /// Prove one exact native OPL2 musical realization on QEMU AdLib.
    Opl2Proof,
}

#[derive(Args, Debug, Clone, Copy)]
struct TargetArgs {
    /// Architecture backend selected explicitly from the pinned Limine matrix.
    #[arg(long, value_enum, default_value_t = ConduitosArch::X86_64)]
    arch: ConduitosArch,

    /// Exact BCM2835 board profile for ARMv6 image and build commands.
    #[arg(long, value_enum)]
    board: Option<armv6_rpi_board::Armv6RpiBoard>,
}

#[derive(Args, Debug, Clone)]
struct FlashArgs {
    /// Architecture image to write; ARMv6 Raspberry Pi is currently supported.
    #[arg(long, value_enum)]
    arch: ConduitosArch,

    /// Exact BCM2835 board profile to image and write.
    #[arg(long, value_enum)]
    board: Option<armv6_rpi_board::Armv6RpiBoard>,

    /// Exact whole removable block device to erase and write.
    #[arg(long)]
    device: PathBuf,

    /// Repeat the exact device path to acknowledge destructive erasure.
    #[arg(long)]
    confirm_device: PathBuf,
}

#[derive(Args, Debug, Clone)]
struct RpiPhysicalProofArgs {
    /// Exact BCM2835 board expected on the UART attachment.
    #[arg(long, value_enum)]
    board: armv6_rpi_board::Armv6RpiBoard,

    /// Exact UART character device connected to GPIO 14/15 through 3.3V TTL.
    #[arg(long)]
    serial_device: PathBuf,

    /// Finite capture deadline in seconds.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120))]
    timeout_seconds: u64,
}

#[derive(Args, Debug, Clone, Copy)]
struct DemoArgs {
    /// Architecture with an implemented visible display and input entrance.
    #[arg(long, value_enum, default_value_t = ConduitosDemoArch::X86_64)]
    arch: ConduitosDemoArch,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ConduitosDemoArch {
    X86_64,
}

impl From<ConduitosDemoArch> for ConduitosArch {
    fn from(value: ConduitosDemoArch) -> Self {
        match value {
            ConduitosDemoArch::X86_64 => Self::X86_64,
        }
    }
}

#[derive(Args, Debug, Clone)]
struct ProveArgs {
    /// Architecture backend selected explicitly from the pinned Limine matrix.
    #[arg(long, value_enum, default_value_t = ConduitosArch::X86_64)]
    arch: ConduitosArch,

    /// Emit bounded proof-native console evidence beneath this root.
    #[arg(long)]
    evidence_root: Option<PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitosArch {
    Ia32,
    X86_64,
    Aarch64,
    Armv6,
    Riscv64,
    Loongarch64,
}

pub(crate) use armv6_rpi_board::Armv6RpiBoard;
pub(crate) use target_build::{build_profile_image, ProfileBuiltImage};

pub(crate) fn build_rpi_image(
    board: Armv6RpiBoard,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    armv6_rpi_b_plus_image::execute(board, opts)
}

pub(crate) fn flash_rpi_image(
    board: Armv6RpiBoard,
    device: &std::path::Path,
    confirm_device: &std::path::Path,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    armv6_rpi_flash::execute(board, device, confirm_device, opts)
}

pub(crate) fn prove_physical_rpi(
    board: Armv6RpiBoard,
    serial_device: &std::path::Path,
    timeout_seconds: u64,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    armv6_rpi_physical::execute(board, serial_device, timeout_seconds, opts)
}

impl ConduitosArch {
    const ALL: [Self; 6] = [
        Self::Ia32,
        Self::X86_64,
        Self::Aarch64,
        Self::Armv6,
        Self::Riscv64,
        Self::Loongarch64,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ia32 => "ia32",
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::Armv6 => "armv6",
            Self::Riscv64 => "riscv64",
            Self::Loongarch64 => "loongarch64",
        }
    }

    fn require_compile_link_backend(self) -> Result<(), ConduitosError> {
        if matches!(
            self,
            Self::Ia32
                | Self::X86_64
                | Self::Aarch64
                | Self::Armv6
                | Self::Riscv64
                | Self::Loongarch64
        ) {
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

    fn require_boot_backend(self) -> Result<(), ConduitosError> {
        if matches!(
            self,
            Self::Ia32
                | Self::X86_64
                | Self::Aarch64
                | Self::Armv6
                | Self::Riscv64
                | Self::Loongarch64
        ) {
            Ok(())
        } else {
            Err(ConduitosError::refusal(
                "unsupported-architecture-boot-backend",
                format!("{} has no accepted ConduitOS boot backend", self.as_str()),
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
        ConduitosCommand::ProductReadinessMatrix => product_readiness_matrix::execute(opts),
        ConduitosCommand::Build(target) => {
            target.arch.require_compile_link_backend()?;
            if target.arch == ConduitosArch::Armv6 {
                armv6_rpi_b_plus_a0::execute(target.board.unwrap_or_default(), opts).map(|_| ())
            } else {
                reject_board_for_non_armv6(target.arch, target.board)?;
                build::execute(target.arch, opts).map(|_| ())
            }
        }
        ConduitosCommand::Image(target) => {
            target.arch.require_boot_backend()?;
            if target.arch == ConduitosArch::Armv6 {
                armv6_rpi_b_plus_image::execute(target.board.unwrap_or_default(), opts)
            } else {
                reject_board_for_non_armv6(target.arch, target.board)?;
                image::execute(target.arch, opts).map(|_| ())
            }
        }
        ConduitosCommand::Flash(flash) => {
            if flash.arch == ConduitosArch::Armv6 {
                armv6_rpi_flash::execute(
                    flash.board.unwrap_or_default(),
                    &flash.device,
                    &flash.confirm_device,
                    opts,
                )
            } else {
                Err(ConduitosError::refusal(
                    "unsupported-flash-target",
                    format!(
                        "{} has no guarded physical flash backend",
                        flash.arch.as_str()
                    ),
                ))
            }
        }
        ConduitosCommand::RpiPhysicalProof(proof) => armv6_rpi_physical::execute(
            proof.board,
            &proof.serial_device,
            proof.timeout_seconds,
            opts,
        ),
        ConduitosCommand::Demo(target) => demo::execute(target.arch.into(), opts),
        ConduitosCommand::FrontDoorProof => front_door_proof::execute(opts),
        ConduitosCommand::JourneyProof => journey_proof::execute(opts),
        ConduitosCommand::Run(target) => {
            target.arch.require_boot_backend()?;
            reject_board_for_non_armv6(target.arch, target.board)?;
            match target.arch {
                ConduitosArch::Aarch64 => aarch64_a1::run(opts),
                ConduitosArch::Armv6 => {
                    armv6_rpi_b_plus_run::execute(target.board.unwrap_or_default(), opts)
                }
                ConduitosArch::Ia32 => ia32_a1::run(opts),
                ConduitosArch::Riscv64 => riscv64_a4::run(opts),
                ConduitosArch::Loongarch64 => loongarch64_a4::run(opts),
                _ => run::execute(target.arch, opts).map(|_| ()),
            }
        }
        ConduitosCommand::Prove(prove_args) => {
            prove_args.arch.require_boot_backend()?;
            if prove_args.evidence_root.is_some() && prove_args.arch != ConduitosArch::X86_64 {
                return Err(ConduitosError::refusal(
                    "unsupported-evidence-architecture",
                    "proof-native ConduitOS evidence currently owns only the x86_64 emulator rung",
                ));
            }
            if prove_args.arch == ConduitosArch::Riscv64 {
                riscv64_a4::prove(opts)
            } else if prove_args.arch == ConduitosArch::Loongarch64 {
                loongarch64_a4::prove(opts)
            } else {
                prove::execute(prove_args.arch, prove_args.evidence_root.as_deref(), opts)
            }
        }
        ConduitosCommand::StdGap => std_gap::execute(opts),
        ConduitosCommand::TimingProfile => timing_profile::execute(opts),
        ConduitosCommand::XhciProof => xhci_proof::execute(opts),
        ConduitosCommand::UsbProof => usb_proof::execute(opts),
        ConduitosCommand::HidProof => hid_proof::execute(opts),
        ConduitosCommand::KeyboardProof => keyboard_proof::execute(opts),
        ConduitosCommand::PcSpeakerProof => pc_speaker_proof::execute(opts),
        ConduitosCommand::HotplugProof => hotplug_proof::execute(opts),
        ConduitosCommand::RescueProof => rescue_proof::execute(opts),
        ConduitosCommand::Opl2Proof => opl2_proof::execute(opts),
    }
}

fn reject_board_for_non_armv6(
    arch: ConduitosArch,
    board: Option<armv6_rpi_board::Armv6RpiBoard>,
) -> Result<(), ConduitosError> {
    if board.is_some() && arch != ConduitosArch::Armv6 {
        return Err(ConduitosError::refusal(
            "board-architecture-mismatch",
            format!("--board is not valid for {}", arch.as_str()),
        ));
    }
    Ok(())
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
    fn visible_demo_is_an_explicit_conduitos_entrance() {
        let parsed =
            Cli::try_parse_from(["xtask", "conduitos", "demo", "--arch", "x86-64"]).unwrap();
        assert!(matches!(parsed.command, Command::Conduitos(_)));
        let error =
            Cli::try_parse_from(["xtask", "conduitos", "demo", "--arch", "aarch64"]).unwrap_err();
        assert!(error.to_string().contains("x86-64"));
    }

    #[test]
    fn added_compile_link_backends_are_explicit() {
        ConduitosArch::Riscv64
            .require_compile_link_backend()
            .unwrap();
        ConduitosArch::Loongarch64
            .require_compile_link_backend()
            .unwrap();
    }

    #[test]
    fn aarch64_has_a_bounded_boot_backend() {
        ConduitosArch::Aarch64
            .require_compile_link_backend()
            .unwrap();
        ConduitosArch::Aarch64.require_boot_backend().unwrap();
    }

    #[test]
    fn ia32_has_a_bounded_boot_backend() {
        ConduitosArch::Ia32.require_compile_link_backend().unwrap();
        ConduitosArch::Ia32.require_boot_backend().unwrap();
    }

    #[test]
    fn loongarch64_has_a_bounded_boot_backend() {
        ConduitosArch::Loongarch64
            .require_compile_link_backend()
            .unwrap();
        ConduitosArch::Loongarch64.require_boot_backend().unwrap();
    }
}
