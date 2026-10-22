//! Explicit installed-owner proof arguments, including isolated mutation guards.
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub(super) struct LiveOwnerActionProofArgs {
    /// Private ISO produced by `make body provision-conduitos` for a live owner.
    #[arg(long)]
    pub(super) spore: PathBuf,
    /// Exact routed candidate ID contained in that ISO.
    #[arg(long)]
    pub(super) candidate_id: String,
    /// Explicit private IPv4 TLS owner listener to receive QEMU guestfwd traffic.
    #[arg(long)]
    pub(super) owner_forward: std::net::SocketAddr,
    /// Existing empty private directory for QMP, receipts, and before/after PNGs.
    #[arg(long)]
    pub(super) output_dir: PathBuf,
    /// Pause at both Face checkpoints for a live browser participant.
    #[arg(long)]
    pub(super) coordinate: bool,
}

#[derive(Args, Debug, Clone)]
pub(super) struct LiveOwnerTodoFaceProofArgs {
    #[command(flatten)]
    pub(super) route: LiveOwnerActionProofArgs,
    /// Exercise three Look selections after activation within four owner refreshes.
    #[arg(long)]
    pub(super) look_selection: bool,
    /// Body expected in the exact installed Owner admission receipt.
    #[arg(long)]
    pub(super) expected_body_id: String,
    /// Installed product executable that owns this Body.
    #[arg(long)]
    pub(super) owner_conduit_bin: PathBuf,
    /// Installed state directory for an independent current Face read.
    #[arg(long)]
    pub(super) owner_state_dir: PathBuf,
    /// Expected number of Todo item subjects in the current Face.
    #[arg(long)]
    pub(super) expected_item_count: usize,
    /// Expected human-facing progress text in the current Todo Face.
    #[arg(long)]
    pub(super) expected_status: String,
}

#[derive(Args, Debug, Clone)]
pub(super) struct LiveOwnerTodoActionProofArgs {
    /// Exercise required-item completion and its reachable generic fallback.
    /// Requires the isolated action fork; uses at most four owner refreshes.
    #[arg(long)]
    pub(super) required_item_fallback: bool,
    /// Remaining offered action to enact through the generic fallback before comparison.
    #[arg(long, requires = "required_item_fallback")]
    pub(super) fallback_action_id: Option<String>,
    /// Tab count from the freshly acknowledged generic fallback to that remaining action.
    #[arg(long, requires = "required_item_fallback")]
    pub(super) fallback_tab_count: Option<u8>,
    /// Expected authoritative status after the remaining action.
    #[arg(long, requires = "required_item_fallback")]
    pub(super) fallback_status: Option<String>,
    #[command(flatten)]
    pub(super) face: LiveOwnerTodoFaceProofArgs,
    /// Exact installed checkpoint directory of a disposable, isolated fork.
    #[arg(long)]
    pub(super) isolated_checkpoint_root: PathBuf,
    /// Exact selected checkpoint version from the installed Owner selection.
    #[arg(long)]
    pub(super) selected_checkpoint_version_hex: String,
    /// Original checkpoint directory to inventory before and after the action.
    #[arg(long)]
    pub(super) protected_checkpoint_root: PathBuf,
    /// Exact available Todo action to require in the Owner Face and serial receipt.
    #[arg(long)]
    pub(super) expected_action_id: String,
    /// Bounded Tab presses to reach that action through the native Mask.
    #[arg(long)]
    pub(super) tab_count: u8,
    /// Expected human progress text after the Owner accepts the action.
    #[arg(long)]
    pub(super) expected_after_status: String,
}
