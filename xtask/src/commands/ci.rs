mod impact;
mod integration;
#[cfg(test)]
mod monitor;
mod product_reconciliation;
mod proof_graph;
mod standalone_locks;

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct CiArgs {
    #[command(subcommand)]
    command: CiCommand,
}

#[derive(Subcommand, Debug)]
enum CiCommand {
    /// Fail fast when a separately rooted fabrication lock is stale.
    StandaloneLocks,
    /// Plan heavyweight CI obligations for one exact Git diff.
    Plan {
        /// Exact base commit SHA.
        base: String,
        /// Exact head commit SHA.
        head: String,
        /// Write the complete machine-readable plan to this path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Write a Markdown job-summary table to this path.
        #[arg(long)]
        summary_out: Option<PathBuf>,
    },
    /// Project exact reconciliation proof IDs onto the check workflow graph.
    ExecutionPlan {
        /// Canonical JSON array of exact proof IDs requiring execution.
        #[arg(long)]
        proof_ids_json: String,
    },
    /// Resolve the exact prospective integration tree without rewriting either input.
    Integration {
        /// Current target-branch commit SHA or ref.
        base: String,
        /// Exact immutable candidate commit SHA or ref.
        head: String,
        /// Write the machine-readable integration result to this path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Write a Markdown summary to this path.
        #[arg(long)]
        summary_out: Option<PathBuf>,
    },
    /// Plan proofs for one immutable candidate head.
    Candidate {
        /// Exact candidate commit SHA or ref.
        head: String,
        /// Previously retained proof receipts to consider, if any.
        #[arg(long = "receipt")]
        receipts: Vec<PathBuf>,
        /// Exact typed impact plan selecting applicable candidate proofs.
        #[arg(long)]
        impact_plan: Option<PathBuf>,
        /// Write the machine-readable candidate plan to this path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Write a Markdown summary to this path.
        #[arg(long)]
        summary_out: Option<PathBuf>,
    },
    /// Reconcile an immutable candidate with a current target base.
    Reconcile {
        /// Current target-branch commit SHA or ref.
        base: String,
        /// Exact immutable candidate commit SHA or ref.
        head: String,
        /// Previously retained proof receipts to consider, if any.
        #[arg(long = "receipt")]
        receipts: Vec<PathBuf>,
        /// Retained exact candidate impact plan selecting applicable proofs.
        #[arg(long)]
        impact_plan: Option<PathBuf>,
        /// Write the machine-readable reconciliation plan to this path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Write a Markdown summary to this path.
        #[arg(long)]
        summary_out: Option<PathBuf>,
    },
    /// Reconcile one fabricated product against an exact integration tree.
    ReconcileProduct {
        /// Registered product proof identifier.
        product_id: String,
        /// Commit whose exact artifact was fabricated and proved.
        candidate: String,
        /// Exact integrated commit proposed for promotion.
        integration: String,
        /// Write the machine-readable reconciliation to this path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Return the execute disposition for an orchestrator to schedule.
        #[arg(long)]
        allow_execute: bool,
    },
    /// Record a successful proof only after its command has completed.
    AttestSuccess {
        /// Exact candidate commit SHA or ref that was proved.
        head: String,
        /// Registered proof identifier whose command just succeeded.
        proof_id: String,
        /// Non-empty machine-readable or human-readable evidence locator.
        #[arg(long)]
        evidence: Vec<String>,
        /// Write the canonical receipt to this path.
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(args: CiArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        CiCommand::StandaloneLocks => standalone_locks::run(),
        CiCommand::Plan {
            base,
            head,
            json_out,
            summary_out,
        } => impact::run(&base, &head, json_out.as_deref(), summary_out.as_deref()),
        CiCommand::ExecutionPlan { proof_ids_json } => proof_graph::execution_plan(&proof_ids_json),
        CiCommand::Candidate {
            head,
            receipts,
            impact_plan,
            json_out,
            summary_out,
        } => proof_graph::candidate(
            &head,
            &receipts,
            impact_plan.as_deref(),
            json_out.as_deref(),
            summary_out.as_deref(),
        ),
        CiCommand::Integration {
            base,
            head,
            json_out,
            summary_out,
        } => integration::run(&base, &head, json_out.as_deref(), summary_out.as_deref()),
        CiCommand::Reconcile {
            base,
            head,
            receipts,
            impact_plan,
            json_out,
            summary_out,
        } => proof_graph::reconcile(
            &base,
            &head,
            &receipts,
            impact_plan.as_deref(),
            json_out.as_deref(),
            summary_out.as_deref(),
        ),
        CiCommand::ReconcileProduct {
            product_id,
            candidate,
            integration,
            json_out,
            allow_execute,
        } => product_reconciliation::run(
            &product_id,
            &candidate,
            &integration,
            json_out.as_deref(),
            allow_execute,
        ),
        CiCommand::AttestSuccess {
            head,
            proof_id,
            evidence,
            out,
        } => proof_graph::attest_success(&head, &proof_id, &evidence, &out),
    }
}
