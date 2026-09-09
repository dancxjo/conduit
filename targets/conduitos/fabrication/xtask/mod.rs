mod aarch64_a0;
mod aarch64_a1;
mod acceptance;
mod active_rescue_proof;
mod architecture_matrix;
#[path = "../../../raspberry-pi/fabrication/xtask/armv6_rpi_b_plus_a0.rs"]
mod armv6_rpi_b_plus_a0;
#[path = "../../../raspberry-pi/fabrication/xtask/armv6_rpi_b_plus_image.rs"]
mod armv6_rpi_b_plus_image;
#[path = "../../../raspberry-pi/fabrication/xtask/armv6_rpi_b_plus_run.rs"]
mod armv6_rpi_b_plus_run;
#[path = "../../../raspberry-pi/fabrication/xtask/armv6_rpi_board.rs"]
mod armv6_rpi_board;
#[path = "../../../raspberry-pi/fabrication/xtask/armv6_rpi_flash.rs"]
mod armv6_rpi_flash;
#[path = "../../../raspberry-pi/fabrication/xtask/armv6_rpi_physical.rs"]
mod armv6_rpi_physical;
mod build;
mod demo;
mod fabrication_resolution;
mod front_door_proof;
mod headless_proof;
mod hid_proof;
mod hid_qmp;
mod hid_run;
mod hotplug_proof;
mod hotplug_qmp;
mod ia32_a0;
mod ia32_a1;
mod ia32_a2;
mod ia32_physical_proof;
mod ia32_product_boot;
mod ia32_vga_receipt;
mod image;
mod isolation_proof;
mod journey_input;
mod journey_pointer;
mod journey_proof;
mod journey_records;
mod journey_resize;
mod journey_standing;
mod journey_tour;
mod journey_transient;
mod journey_usb_line;
mod journey_workset;
mod keyboard_proof;
mod keyboard_repeat_proof;
mod keyboard_run;
mod keyboard_text_run;
mod live_media;
mod loongarch64_a0;
#[allow(dead_code)]
mod loongarch64_a1;
#[allow(dead_code)]
mod loongarch64_a2;
#[allow(dead_code)]
mod loongarch64_a3;
mod loongarch64_a4;
mod loongarch64_product_boot;
mod opl2_proof;
#[path = "../../../orange-pi/fabrication/xtask/orange_pi_5_image.rs"]
mod orange_pi_5_image;
#[path = "../../../orange-pi/fabrication/xtask/orange_pi_5_media.rs"]
mod orange_pi_5_media;
mod pc_speaker_proof;
mod prepared_proof_image;
mod product_readiness_matrix;
mod profile;
mod prove;
mod prove_many;
mod ps2_input_proof;
mod qemu_artifacts;
mod qmp;
mod qmp_display;
mod removable_media;
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
mod riscv64_product_boot;
mod run;
mod std_gap;
pub(crate) mod target_backend;
pub(crate) mod target_build;
mod target_lowering;
mod timing_profile;
mod usb_proof;
mod usb_run;
mod x86_64_product_boot;
mod xhci_proof;

use std::{fmt, path::PathBuf};

use clap::{Args, Subcommand, ValueEnum};

use crate::cli::GlobalOpts;
use fabrication_resolution::{reject_board_for_non_armv6, require_fabrication_target};

#[derive(Args, Debug)]
pub struct ConduitosArgs {
    #[command(subcommand)]
    command: ConduitosCommand,
}

#[derive(Subcommand, Debug)]
enum ConduitosCommand {
    /// Boot one exact Crèche-exported ConduitOS spore through the product journey.
    Acceptance(AcceptanceArgs),
    /// Verify and report the pinned Limine architecture/backend matrix.
    ArchitectureMatrix,
    /// Report exact earned Product Spine cells independently of A0-A4.
    ProductReadinessMatrix,
    /// Compile and mechanically inspect one bounded architecture proof appliance.
    Build(TargetArgs),
    /// Package one bounded architecture proof appliance into its pinned boot image.
    Image(TargetArgs),
    /// Build canonical bootable media for one normal ConduitOS product Host.
    Live(LiveArgs),
    /// Boot the canonical live artifact without building a parallel demo image.
    LiveBoot(LiveArgs),
    /// Prove the canonical IA-32 live artifact through legacy BIOS only.
    Ia32LegacyBiosProof,
    /// Seal two attended physical Mabel boots of one byte-verified IA-32 medium.
    Ia32MabelPhysicalProof(ia32_physical_proof::Args),
    /// Report every current live artifact and every excluded capability gap.
    LiveMatrix,
    /// Build one exact bare-metal ConduitOS Orange Pi 5 RK3588S SD image.
    OrangePi5Image,
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
    /// Verify an exact headless artifact and its explicit unsupported startup contract.
    HeadlessProof { output: PathBuf },
    /// Boot one architecture proof appliance and validate its bounded terminal Sign.
    Run(TargetArgs),
    /// Prove compile/link/image/boot truth and fresh boot identities.
    Prove(ProveArgs),
    /// Execute selected x86 proofs concurrently in one prepared environment.
    ProveMany(prove_many::ProveManyArgs),
    /// Inventory the portable std nucleus and classify the exact ConduitOS gap.
    StdGap,
    /// Prove one exact deterministic deadline-bounded local Plan and refusal.
    TimingProfile,
    /// Prove one real bounded xHCI Base and fail-closed controller absence.
    XhciProof(PreparedProofArgs),
    /// Prove one real bounded root-attached USB device without semantic input.
    UsbProof(PreparedProofArgs),
    /// Prove one real HID boot-keyboard press/release stream without semantics.
    HidProof(PreparedProofArgs),
    /// Prove the exact portable keyboard offer, Plan, Play, and event values.
    KeyboardProof(PreparedProofArgs),
    /// Prove repeated normal-product input across several fixed USB keyboard ring cycles.
    KeyboardRepeatProof,
    /// Prove bounded PS/2 keyboard and pointer input through ordinary product semantics.
    Ps2InputProof,
    /// Build one immutable x86 proof image for bounded downstream PLAY jobs.
    PrepareProofImage,
    /// Prove the exact PC-speaker offer, Plan, production-kernel Play, and Base effects.
    PcSpeakerProof,
    /// Prove real USB keyboard detach/reattach across immutable and fresh Plans.
    HotplugProof,
    /// Prove one low-level local rescue request and real fresh boot.
    RescueProof(PreparedProofArgs),
    /// Prove one exact native OPL2 musical realization on QEMU AdLib.
    Opl2Proof,
    /// Prove one x86_64 ring-3 protection domain and exact kernel capability gate.
    IsolationProof,
}

#[derive(Args, Debug, Clone)]
struct AcceptanceArgs {
    /// Exact Body-provisioned ConduitOS ISO exported by the Crèche.
    #[arg(long)]
    spore: PathBuf,
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
    /// Architecture image to write; IA-32 live media and ARMv6 Raspberry Pi are supported.
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

#[derive(Args, Debug, Clone, Copy)]
struct LiveArgs {
    /// Exact current ConduitOS product Host type.
    #[arg(value_enum, default_value_t = live_media::LiveHost::default())]
    host: live_media::LiveHost,
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

#[derive(Args, Debug, Clone, Copy)]
struct PreparedProofArgs {
    /// Verify and play the exact image prepared by `prepare-proof-image`.
    #[arg(long)]
    prepared_image: bool,
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
pub(crate) use report::ArtifactRole;
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
        ConduitosCommand::Acceptance(args) => acceptance::execute(&args.spore, opts),
        ConduitosCommand::ArchitectureMatrix => architecture_matrix::execute(opts),
        ConduitosCommand::ProductReadinessMatrix => product_readiness_matrix::execute(opts),
        ConduitosCommand::Build(target) => {
            target.arch.require_compile_link_backend()?;
            require_fabrication_target(target.arch, target.board)?;
            if target.arch == ConduitosArch::Armv6 {
                armv6_rpi_b_plus_a0::execute(target.board.unwrap_or_default(), opts).map(|_| ())
            } else {
                reject_board_for_non_armv6(target.arch, target.board)?;
                build::execute_architecture_proof(target.arch, opts).map(|_| ())
            }
        }
        ConduitosCommand::Image(target) => {
            target.arch.require_boot_backend()?;
            require_fabrication_target(target.arch, target.board)?;
            if target.arch == ConduitosArch::Armv6 {
                armv6_rpi_b_plus_image::execute(target.board.unwrap_or_default(), opts)
            } else {
                reject_board_for_non_armv6(target.arch, target.board)?;
                image::execute_architecture_proof(target.arch, opts).map(|_| ())
            }
        }
        ConduitosCommand::Live(args) => live_media::build(args.host, opts),
        ConduitosCommand::LiveBoot(args) => live_media::boot(args.host, opts),
        ConduitosCommand::Ia32LegacyBiosProof => live_media::prove_ia32_legacy_bios(opts),
        ConduitosCommand::Ia32MabelPhysicalProof(args) => ia32_physical_proof::execute(&args, opts),
        ConduitosCommand::LiveMatrix => live_media::matrix(opts),
        ConduitosCommand::OrangePi5Image => orange_pi_5_image::execute(opts),
        ConduitosCommand::Flash(flash) => {
            require_fabrication_target(flash.arch, flash.board)?;
            match flash.arch {
                ConduitosArch::Armv6 => armv6_rpi_flash::execute(
                    flash.board.unwrap_or_default(),
                    &flash.device,
                    &flash.confirm_device,
                    opts,
                ),
                ConduitosArch::Ia32 => {
                    live_media::flash_ia32(&flash.device, &flash.confirm_device, opts)
                }
                _ => Err(ConduitosError::refusal(
                    "unsupported-flash-target",
                    format!(
                        "{} has no guarded physical flash backend",
                        flash.arch.as_str()
                    ),
                )),
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
        ConduitosCommand::HeadlessProof { output } => headless_proof::execute(output, opts),
        ConduitosCommand::Run(target) => {
            target.arch.require_boot_backend()?;
            require_fabrication_target(target.arch, target.board)?;
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
        ConduitosCommand::ProveMany(args) => prove_many::execute(args, opts),
        ConduitosCommand::StdGap => std_gap::execute(opts),
        ConduitosCommand::TimingProfile => timing_profile::execute(opts),
        ConduitosCommand::XhciProof(args) => xhci_proof::execute(args.prepared_image, opts),
        ConduitosCommand::UsbProof(args) => usb_proof::execute(args.prepared_image, opts),
        ConduitosCommand::HidProof(args) => hid_proof::execute(args.prepared_image, opts),
        ConduitosCommand::KeyboardProof(args) => keyboard_proof::execute(args.prepared_image, opts),
        ConduitosCommand::KeyboardRepeatProof => keyboard_repeat_proof::execute(opts),
        ConduitosCommand::Ps2InputProof => ps2_input_proof::execute(opts),
        ConduitosCommand::PrepareProofImage => prepared_proof_image::prepare(opts),
        ConduitosCommand::PcSpeakerProof => pc_speaker_proof::execute(opts),
        ConduitosCommand::HotplugProof => hotplug_proof::execute(opts),
        ConduitosCommand::RescueProof(args) => rescue_proof::execute(args.prepared_image, opts),
        ConduitosCommand::Opl2Proof => opl2_proof::execute(opts),
        ConduitosCommand::IsolationProof => isolation_proof::execute(opts),
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
    fn visible_demo_is_an_explicit_conduitos_entrance() {
        let parsed =
            Cli::try_parse_from(["xtask", "conduitos", "demo", "--arch", "x86-64"]).unwrap();
        assert!(matches!(parsed.command, Command::Conduitos(_)));
        let error =
            Cli::try_parse_from(["xtask", "conduitos", "demo", "--arch", "aarch64"]).unwrap_err();
        assert!(error.to_string().contains("x86-64"));
    }

    #[test]
    fn live_media_build_boot_and_matrix_are_memorable_typed_entrances() {
        for arguments in [
            vec!["xtask", "conduitos", "live"],
            vec!["xtask", "conduitos", "live", "x86_64"],
            vec!["xtask", "conduitos", "live", "riscv64"],
            vec!["xtask", "conduitos", "live-boot", "aarch64"],
            vec!["xtask", "conduitos", "ia32-legacy-bios-proof"],
            vec!["xtask", "conduitos", "live-matrix"],
        ] {
            let parsed = Cli::try_parse_from(arguments).unwrap();
            assert!(matches!(parsed.command, Command::Conduitos(_)));
        }
    }

    #[test]
    fn ia32_flash_requires_one_explicit_repeated_whole_device() {
        let parsed = Cli::try_parse_from([
            "xtask",
            "conduitos",
            "flash",
            "--arch",
            "ia32",
            "--device",
            "/dev/sdz",
            "--confirm-device",
            "/dev/sdz",
        ]);
        assert!(parsed.is_ok());

        let missing_confirmation = Cli::try_parse_from([
            "xtask",
            "conduitos",
            "flash",
            "--arch",
            "ia32",
            "--device",
            "/dev/sdz",
        ]);
        assert!(missing_confirmation.is_err());
    }

    #[test]
    fn mabel_physical_proof_requires_two_attended_boots() {
        let parsed = Cli::try_parse_from([
            "xtask",
            "conduitos",
            "ia32-mabel-physical-proof",
            "--flash-record",
            "flash.json",
            "--first-photo",
            "first.jpg",
            "--first-host-id",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--first-boot-id",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "--second-photo",
            "second.jpg",
            "--second-host-id",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "--second-boot-id",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "--confirm-image-sha256",
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "--confirm-specimen",
            "mabel-copperbutton",
            "--attest-exact-screens",
        ]);
        assert!(parsed.is_ok());
    }

    #[test]
    fn prepared_image_build_and_play_flags_are_explicit() {
        let build = Cli::try_parse_from(["xtask", "conduitos", "prepare-proof-image"]);
        assert!(build.is_ok());
        for proof in [
            "xhci-proof",
            "usb-proof",
            "hid-proof",
            "keyboard-proof",
            "rescue-proof",
        ] {
            let play = Cli::try_parse_from(["xtask", "conduitos", proof, "--prepared-image"]);
            assert!(play.is_ok(), "{proof}");
        }
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
