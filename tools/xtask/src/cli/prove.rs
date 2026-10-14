use super::{AudioPlaybackArgs, DemoArgs};
use crate::commands::{
    body_coordination::BodyCoordinationArgs, evidence::EvidenceCommand, pete_std_observe::PeteArgs,
};
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum ProveCommand {
    /// Retain release evaluator and production shared-pool selection baselines.
    RuntimePerformance {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
    /// Prove the hosted Burn authoring and checkpoint contracts.
    ModelAuthoring {
        #[arg(long, conflicts_with = "cuda")]
        documented_command: bool,
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        #[arg(long)]
        cuda: bool,
    },
    /// Run the bounded audible specimen through one exact selected output.
    AudioPlayback(AudioPlaybackArgs),
    /// Prove bounded Pete forebrain-motherbrain coordination.
    BodyCoordination(BodyCoordinationArgs),
    /// Exercise explicit Pete hardware proof entrances.
    Pete(PeteArgs),
    /// Exercise one reviewed Plot or journey through its exact repository proof path.
    Journey(DemoArgs),
    /// Capture a live, partial Todo owner/terminal encounter without publishing a journey.
    TodoJourney(TodoJourneyArgs),
    /// Produce, verify, or publish bounded proof evidence.
    #[command(flatten)]
    Evidence(EvidenceCommand),
}

#[derive(Args, Debug)]
pub struct TodoJourneyArgs {
    /// Installed owner state whose current Face will be observed.
    #[arg(long)]
    pub state_dir: std::path::PathBuf,
    /// Executable from the reviewed Host release that owns this installation.
    #[arg(long)]
    pub conduit_bin: std::path::PathBuf,
    /// New directory in which to retain the exact command outputs.
    #[arg(long)]
    pub output: std::path::PathBuf,
    /// UTF-8 terminal commands (defaults to one read followed by quit).
    #[arg(long)]
    pub terminal_script: Option<std::path::PathBuf>,
    /// Birth a new Todo Body in this installed, unowned Host before capturing its first browser Add.
    #[arg(long)]
    pub fresh_body_source: Option<std::path::PathBuf>,
    /// Exact-source static Handbook package for the fresh browser Add.
    #[arg(long)]
    pub handbook_package: Option<std::path::PathBuf>,
    /// Pinned Playwright module used by the browser Add producer.
    #[arg(long)]
    pub pinned_playwright: Option<std::path::PathBuf>,
    /// First item text for the fresh browser Add (defaults to Buy milk).
    #[arg(long)]
    pub first_item_text: Option<String>,
    /// Add three browser items, complete one through the installed terminal, then observe it in the browser.
    #[arg(long)]
    pub cross_mask_actions: bool,
    /// Capture actual owner Todo Faces at 0, 1, 3 and 20 remaining items.
    #[arg(long, conflicts_with_all = ["cross_mask_actions", "direct_speech"])]
    pub count_matrix: bool,
    /// Capture acknowledged direct opening and remaining-item WAVs through the selected artifact route.
    #[arg(long, requires = "cross_mask_actions")]
    pub direct_speech: bool,
}
