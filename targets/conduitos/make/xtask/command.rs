//! Command topology; concrete argument contracts remain with their owners.
use super::*;
use clap::Subcommand;

#[derive(Args, Debug)]
pub struct ConduitosArgs {
    #[command(subcommand)]
    pub(super) command: ConduitosCommand,
}

#[derive(Subcommand, Debug)]
pub(super) enum ConduitosCommand {
    /// Boot one exact Crèche-exported ConduitOS spore through the product journey.
    Acceptance(AcceptanceArgs),
    /// Check and package bounded Source without building the target.
    ProtocolSource(protocol_source::PackageArgs),
    /// Package a locally approved protocol over an existing capable product kernel.
    ProtocolImage(protocol_image::ImageArgs),
    /// Encode one input with the exact checked entry schema.
    ProtocolInput(protocol_input::InputArgs),
    /// Observe a packaged protocol image in a bounded isolated emulator run.
    ProtocolRun(protocol_run::RunArgs),
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
    /// Boot one private owner-provisioned x86 product ISO with its exact VirtIO route.
    LiveOwnerBoot(LiveOwnerBootArgs),
    /// Prove one typed native clock action through a live installed owner.
    LiveOwnerActionProof(LiveOwnerActionProofArgs),
    /// Prove one Body across an installed owner, QMP guest, and pinned Chromium.
    LiveThreeHostProof(Box<three_host_proof::Args>),
    /// Birth one Body through the installed screen-free client, then prove three live Hosts.
    ScreenFreeThreeHostProof(Box<screen_free_three_host_proof::Args>),
    /// Hold one private model route for installed-owner provider-loss proof.
    OwnerModelRoute(owner_model_route::Args),
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
    /// Prove the normal IMAGE zero-body front door and long-lived interaction.
    FrontDoorProof,
    /// Prove the normal IMAGE Body/Wake/Plan/Play product journey.
    JourneyProof,
    /// Prove the default graphical profile through the canonical live ISO and retained gallery.
    GraphicalProfileProof,
    /// Boot one architecture proof appliance and validate its bounded terminal Sign.
    Run(TargetArgs),
    /// Prove compile/link/image/boot truth and fresh boot identities.
    Prove(ProveArgs),
    /// Execute selected x86 proofs concurrently in one prepared environment.
    ProveMany(prove_many::ProveManyArgs),
    /// Inventory the portable std nucleus and classify the exact ConduitOS gap.
    StdGap,
    /// Prove one exact deterministic deadline-bounded local plan and refusal.
    TimingProfile,
    /// Prove one real bounded xHCI Base and fail-closed controller absence.
    XhciProof(PreparedProofArgs),
    /// Prove one real bounded root-attached USB device without semantic input.
    UsbProof(UsbProofArgs),
    /// Prove the complete Source configuration exchange with its admitted preparation arena.
    UsbConfigurationProof,
    /// Check shared USB wire plots and machine register-possession fixtures.
    UsbPlotsCheck {
        /// Type-check the shared library for each product CPU architecture.
        #[arg(long)]
        cross: bool,
    },
    /// Prove one real HID boot-keyboard press/release stream without semantics.
    HidProof(PreparedProofArgs),
    /// Prove the exact portable keyboard offer, Plan, Play, and event values.
    KeyboardProof(PreparedProofArgs),
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
    /// Prove the x86_64 emergency callback enters a terminal CPU halt.
    EmergencyHaltProof,
    /// Prove one exact native OPL2 musical realization on QEMU AdLib.
    Opl2Proof,
    /// Prove one x86_64 ring-3 protection domain and exact kernel capability gate.
    IsolationProof,
    /// Run checked ordinary text Source with its production kernel and CPL3 implementation.
    OrdinaryDomainProof,
    /// Prove ordinary IA-32 ring-3 execution and independent hostile entries.
    Ia32OrdinaryDomainProof,
    /// Prove ordinary AArch64 EL0 execution and independent boundary checks.
    Aarch64OrdinaryDomainProof,
    /// Prove ordinary RISC-V64 U-mode execution and independent boundary checks.
    Riscv64OrdinaryDomainProof,
    /// Prove ordinary LoongArch64 PLV3 execution and independent boundary checks.
    Loongarch64OrdinaryDomainProof,
    /// Prove one real fixed-ring VirtIO-net exchange with the QEMU gateway.
    VirtioNetProof,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, MakeArgs, MakeTarget};
    use clap::Parser;

    #[test]
    fn protocol_source_command_requires_package_entry_and_output_directory() {
        let command = ["xtask", "make", "conduitos", "protocol-source"];
        assert!(Cli::try_parse_from(command).is_err());
        let cli = Cli::try_parse_from(command.into_iter().chain([
            "--package",
            "source.json",
            "--entry",
            "protocol",
            "--output-dir",
            "checked",
        ]))
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Conduitos(ConduitosArgs {
                    command: ConduitosCommand::ProtocolSource(_),
                }),
            })
        ));
    }
    #[test]
    fn protocol_image_requires_existing_kernel_record_and_both_exact_modules() {
        let command = ["xtask", "make", "conduitos", "protocol-image"];
        let pairs = [
            ["--kernel", "product/conduitos"],
            ["--build-record", "product/build.json"],
            ["--package", "source.json"],
            ["--root-request", "root.json"],
            ["--output-dir", "media"],
        ];
        for missing in 0..pairs.len() {
            let args = command.into_iter().chain(
                pairs
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != missing)
                    .flat_map(|(_, pair)| pair.iter().copied()),
            );
            assert!(Cli::try_parse_from(args).is_err());
        }
        let cli =
            Cli::try_parse_from(command.into_iter().chain(pairs.into_iter().flatten())).unwrap();
        assert!(matches!(
            cli.command,
            Command::Make(MakeArgs {
                target: MakeTarget::Conduitos(ConduitosArgs {
                    command: ConduitosCommand::ProtocolImage(_),
                }),
            })
        ));
    }
    #[test]
    fn source_compilation_requires_a_recipe_and_rejects_mixed_package_input() {
        let args = [
            "xtask",
            "make",
            "conduitos",
            "protocol-source",
            "--source",
            "source.conduit",
            "--entry",
            "protocol",
            "--output-dir",
            "checked",
        ];
        assert!(Cli::try_parse_from(args).is_err());
        assert!(
            Cli::try_parse_from(args.into_iter().chain(["--specializations", "recipe.json"]))
                .is_ok()
        );
        assert!(Cli::try_parse_from(args.into_iter().chain([
            "--specializations",
            "recipe.json",
            "--package",
            "old.json"
        ]))
        .is_err());
    }
}

#[derive(Args, Debug)]
pub(super) struct UsbProofArgs {
    #[arg(long, conflicts_with_all = ["endpoint_read", "hid_endpoint", "hid_mouse"])]
    pub(super) prepared_image: bool,
    /// Run the dedicated class-neutral endpoint-receive proof appliance.
    #[arg(long)]
    pub(super) endpoint_read: bool,
    /// Run the Source keyboard class over the native endpoint owner.
    #[arg(long, conflicts_with = "endpoint_read")]
    pub(super) hid_endpoint: bool,
    /// Run the Source mouse class over the native endpoint owner.
    #[arg(long, conflicts_with_all = ["endpoint_read", "hid_endpoint"])]
    pub(super) hid_mouse: bool,
}

#[cfg(test)]
mod endpoint_command_tests {
    use crate::cli::Cli;
    use clap::Parser;
    #[test]
    fn endpoint_proof_requires_its_own_build_and_refuses_prepared_ordinary_images() {
        assert!(
            Cli::try_parse_from(["xtask", "make", "conduitos", "usb-proof", "--hid-mouse"]).is_ok()
        );
        for conflicting in ["--prepared-image", "--endpoint-read", "--hid-endpoint"] {
            assert!(Cli::try_parse_from([
                "xtask",
                "make",
                "conduitos",
                "usb-proof",
                "--hid-mouse",
                conflicting
            ])
            .is_err());
        }
        assert!(
            Cli::try_parse_from(["xtask", "make", "conduitos", "usb-proof", "--hid-endpoint"])
                .is_ok()
        );
        for conflicting in ["--prepared-image", "--endpoint-read"] {
            assert!(Cli::try_parse_from([
                "xtask",
                "make",
                "conduitos",
                "usb-proof",
                "--hid-endpoint",
                conflicting
            ])
            .is_err());
        }
        assert!(Cli::try_parse_from([
            "xtask",
            "make",
            "conduitos",
            "usb-proof",
            "--endpoint-read"
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "xtask",
            "make",
            "conduitos",
            "usb-proof",
            "--endpoint-read",
            "--prepared-image"
        ])
        .is_err());
    }
}
