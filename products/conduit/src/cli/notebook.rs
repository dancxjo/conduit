use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Local owner controls; presentation Masks use the same continuing document.
#[derive(Debug, Subcommand, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum NotebookCommand {
    /// Initialize a new empty document in an already-born Notebook Body.
    New {
        #[arg(long, default_value = "Thoughts")]
        name: String,
    },
    /// Read the current document and its exact revision.
    Show,
    /// Propose a whole-document replacement against the observed revision.
    Replace {
        #[arg(long)]
        revision: u64,
        #[arg(long)]
        text: String,
    },
    /// Commit the current document beneath this explicitly authorized directory.
    Save {
        #[arg(long)]
        storage_dir: PathBuf,
    },
    /// Reopen the selected committed revision from this authorized directory.
    Reopen {
        #[arg(long)]
        storage_dir: PathBuf,
    },
}
