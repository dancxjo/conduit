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
    UsbProof(PreparedProofArgs),
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
    /// Prove one real fixed-ring VirtIO-net exchange with the QEMU gateway.
    VirtioNetProof,
}

