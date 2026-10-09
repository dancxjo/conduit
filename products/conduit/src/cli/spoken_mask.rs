//! Public controls for the installed owner's spoken Mask.
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum SpokenMaskCommand {
    /// Admit the direct or --llm Face-to-artifact child in the Body Plan.
    Admit {
        #[arg(long)]
        llm: bool,
    },
    /// Wear and select that exact child before its Play.
    Select {
        #[arg(long)]
        llm: bool,
    },
    /// Start one cancellable selected spoken Mask Play.
    Start {
        #[arg(long)]
        llm: bool,
    },
    /// Read the current primary items from the acknowledged direct Show.
    ReadRemaining,
    /// Inspect the exact operation's terminal or running state.
    Status {
        operation_id: String,
        #[arg(long)]
        llm: bool,
    },
    /// Request cancellation of the exact running operation.
    Stop {
        operation_id: String,
        #[arg(long)]
        llm: bool,
    },
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn installed_spoken_mask_accepts_remaining_item_request() {
        let parsed = Cli::try_parse_from([
            "conduit",
            "body",
            "spoken-mask",
            "--state-dir",
            "/tmp/conduit-spoken-detail",
            "read-remaining",
        ]);
        assert!(parsed.is_ok(), "{parsed:?}");
    }
}
