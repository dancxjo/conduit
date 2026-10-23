use clap::{Args, Subcommand, ValueEnum};

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Which check suite to execute (default: workspace).
    pub suite: Option<CheckSuite>,

    /// Inspect or validate one repository-owned semantic scope.
    #[command(subcommand)]
    pub scope: Option<CheckScope>,
}

#[derive(Subcommand, Debug)]
pub enum CheckScope {
    /// Inspect mechanically derived portable kind coverage by Host profile.
    Catalog(crate::commands::catalog::CatalogArgs),
    /// Check and report the explicit reviewed plot inventory.
    Plots(crate::commands::plots::PlotsArgs),
    /// Check Pete's reviewed workload and Host make closure without physical access.
    Pete,
    /// Validate portable device protocol plots and finite hardware contracts.
    DeviceProtocols,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSuite {
    Workspace,
    WorkspaceLint,
    WorkspaceTestFoundation,
    WorkspaceTestHosts,
    WorkspaceTestHostsStd,
    WorkspaceTestHostsBrowser,
    WorkspaceTestHostsConduitos,
    WorkspaceTestHostsWorkbench,
    WorkspaceTestProducts,
    WorkspacePortable,
    WorkspacePico,
    Browser,
    Sim,
    KernelTakeover,
    PlanningS2,
    PlotS3,
    /// Report native expression baselines and execute portable-WASM scalar parity.
    ExpressionBaseline,
    Observatory,
    SemanticCatalog,
    /// Execute authored quantity mappings through the std production kernel and face.
    QuantityMapping,
    /// Check exact acoustic control conversions and installed Source projections.
    AcousticControl,
    /// Prove bounded Todo state transitions and recursive Plot execution.
    TodoState,
    /// Prove selected Todo checkpoint persistence and same-Body Host continuity.
    TodoDurability,
    /// Check canonical IPA notation and first-class phone/phoneme authoring.
    SpeechIpa,
    /// Prove finite categorical request custody, kernel ACKs and retained Sign capacity.
    CategoricalRuntime,
    /// Bind exact canonical checked Source exports to sealed generic kernel Fores.
    CanonicalExport,
    /// Prove imported model admission and prepared named-tensor Source/Plan/Play.
    ModelInference,
    /// Prove an explicit installed CPU profile has finite native backing and fail-closed pressure.
    ModelNativeBacking,
    /// Prove exact Angle-to-radian reference transport and certified numerical realization.
    AngleReference,
    /// Prove first-class physical values, derived quantities and installed Plan/Play.
    PhysicalValues,
    /// Prove exact inverse Angle Source, error bounds and installed kernel execution.
    InverseAngle,
    /// Prove authored finite acoustic state and PCM transport through the production kernel.
    AcousticFlow,
    /// Prove shared speech/music DSP components, finite state and compact-voice parity.
    DspSharedCompositions,
    /// Prove typed fixture printers with the generic shared-pool scheduler.
    PrintPool,
    /// Check the finite parser action artifact and numerical admission.
    ParserScorer,
    /// Download pinned corpus into temporary storage and reproduce the parser artifact.
    ParserScorerReproduce,
    /// Exercise optional admitted parser scoring through ordinary Plan/Play.
    ParserSelection,
    /// Exercise classified ASR revisions through common Language and installed StdHost.
    AsrCommonLanguage,
    /// Check committed dependency/prosody correlation and bounded revision safety.
    ProsodyCommitment,
    /// Check committed noun/verb pronunciation and installed compact PCM.
    RecordPronunciation,
    /// Prove bounded speech selection and explicit detail from exact Faces.
    TodoSpeech,
    InputSemantics,
    /// Prove owner-issued face routes without waking the workload.
    OwnerFace,
    /// Run the Linux Landlock/seccomp hosted Base confinement proof.
    HostedBaseIsolation,
    /// Prove the exact-endpoint OS-capability-mediated HTTP Base.
    IsolatedHttpBase,
    /// Prove directional external mappings and exact reflection fencing.
    InteropMembrane,
    /// Prove mutually authenticated, replay-fenced, non-transitive federation.
    FederationSecurity,
    /// Prove bounded no-WASI execution and capability-scoped hostile Gear refusal.
    ConfinedGear,
    /// Prove generic attended last-mile consequential-effect gating.
    ConsequentialEffect,
    /// Prove the bounded ROS 2 topic Base against native ROS Jazzy.
    Ros2Base,
    /// Run the permanent cross-boundary adversarial security acceptance gate.
    SecurityAcceptance,
    /// Run the attended low-energy physical-effect HIL proof.
    PhysicalEffectHil,
    All,
}
