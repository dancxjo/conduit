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
    WorkspaceTestProducts,
    WorkspacePortable,
    WorkspacePico,
    Browser,
    Sim,
    KernelTakeover,
    PlanningS2,
    PlotS3,
    Observatory,
    SemanticCatalog,
    /// Execute authored quantity mappings through the std production kernel and presentation.
    QuantityMapping,
    /// Prove bounded Todo state transitions and recursive Plot execution.
    TodoState,
    InputSemantics,
    /// Prove owner-issued presentation routes without waking the workload.
    OwnerPresentation,
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
