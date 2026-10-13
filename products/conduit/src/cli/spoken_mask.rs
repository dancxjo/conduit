//! Public controls for the installed owner's spoken Mask.
use clap::{Subcommand, ValueEnum};

/// Finite checked source Look selection; no caller-supplied wording or identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum SpokenTemplateLook {
    Brief,
    Expanded,
}
impl From<SpokenTemplateLook>
    for conduit_std_host::direct_spoken_template_mask::DirectSpokenTemplateLook
{
    fn from(look: SpokenTemplateLook) -> Self {
        match look {
            SpokenTemplateLook::Brief => Self::Brief,
            SpokenTemplateLook::Expanded => Self::Expanded,
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum SpokenMaskCommand {
    /// Admit the direct or --llm Face-to-artifact child in the Body Plan.
    Admit {
        #[arg(long)]
        llm: bool,
        /// Select a checked spoken Look over the current Face.
        #[arg(long, value_enum, conflicts_with = "llm")]
        look: Option<SpokenTemplateLook>,
    },
    /// Wear and select that exact child before its Play.
    Select {
        #[arg(long)]
        llm: bool,
        /// Wear the exact checked spoken Look over the current Face.
        #[arg(long, value_enum, conflicts_with = "llm")]
        look: Option<SpokenTemplateLook>,
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

#[cfg(test)]
mod look_tests {
    use super::{SpokenMaskCommand, SpokenTemplateLook};
    use crate::cli::{BodyCommand, Cli, Command};
    use clap::Parser;

    fn command(operation: &str, options: &[&str]) -> Result<SpokenMaskCommand, clap::Error> {
        let mut args = vec![
            "conduit",
            "body",
            "spoken-mask",
            "--state-dir",
            "/tmp/spoken-look-parser",
            operation,
        ];
        args.extend_from_slice(options);
        let parsed = Cli::try_parse_from(args)?;
        let Some(Command::Body {
            command: Some(BodyCommand::SpokenMask { command, .. }),
        }) = parsed.command
        else {
            panic!("spoken Mask command");
        };
        Ok(command)
    }

    #[test]
    fn installed_spoken_look_selection_maps_only_checked_brief_and_expanded() {
        for (value, expected) in [
            ("brief", SpokenTemplateLook::Brief),
            ("expanded", SpokenTemplateLook::Expanded),
        ] {
            for operation in ["admit", "select"] {
                let command = command(operation, &["--look", value]).unwrap();
                let actual = match command {
                    SpokenMaskCommand::Admit { llm: false, look }
                    | SpokenMaskCommand::Select { llm: false, look } => look,
                    _ => panic!("direct Look selection"),
                };
                assert_eq!(actual, Some(expected));
                let std: conduit_std_host::direct_spoken_template_mask::DirectSpokenTemplateLook =
                    expected.into();
                assert_eq!(
                    std.entry(),
                    if value == "brief" {
                        "spoken/brief-template"
                    } else {
                        "spoken/expanded-template"
                    }
                );
            }
        }
    }

    #[test]
    fn unknown_or_llm_combined_look_refuses_and_default_route_remains_ordinary() {
        for operation in ["admit", "select"] {
            assert!(command(operation, &["--look", "custom"]).is_err());
            assert!(command(operation, &["--look", "brief", "--llm"]).is_err());
            assert!(command(operation, &["--llm", "--look", "expanded"]).is_err());
            assert!(matches!(
                command(operation, &[]).unwrap(),
                SpokenMaskCommand::Admit {
                    llm: false,
                    look: None
                } | SpokenMaskCommand::Select {
                    llm: false,
                    look: None
                }
            ));
            assert!(matches!(
                command(operation, &["--llm"]).unwrap(),
                SpokenMaskCommand::Admit {
                    llm: true,
                    look: None
                } | SpokenMaskCommand::Select {
                    llm: true,
                    look: None
                }
            ));
        }
        assert!(command("start", &["--look", "brief"]).is_err());
    }
}
