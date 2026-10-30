use clap::Args;

/// One reviewed local model offered by every fresh installed Host Boot.
#[derive(Debug, Default, Args)]
pub(crate) struct InstalledModelOptions {
    /// Select an already local Ollama model for this installed Host.
    #[arg(long, requires_all = ["model_endpoint", "model_memory_mib"])]
    pub(crate) selected_model: Option<String>,
    /// Remove the retained model selection on reinstall.
    #[arg(long, conflicts_with = "selected_model")]
    pub(crate) without_selected_model: bool,
    #[arg(long, requires = "selected_model")]
    pub(crate) model_endpoint: Option<String>,
    #[arg(long, requires = "selected_model")]
    pub(crate) model_memory_mib: Option<u32>,
    /// Also offer bounded text generation through this explicitly selected model.
    #[arg(long, requires = "selected_model")]
    pub(crate) model_generation: bool,
    /// Select an exact semantic model profile (for example stream-generate).
    /// Repeat for several profiles; omitted profiles retain the presentation default.
    #[arg(long = "model-profile", value_name = "PROFILE", requires = "selected_model", conflicts_with = "model_generation", value_parser = parse_model_profile)]
    pub(crate) model_profiles: Vec<conduit_std_host::hosted_local_model::LocalModelKindProfile>,
}

fn parse_model_profile(
    value: &str,
) -> Result<conduit_std_host::hosted_local_model::LocalModelKindProfile, String> {
    use conduit_std_host::hosted_local_model::LocalModelKindProfile as P;
    match value {
        "generate" => Ok(P::Generate),
        "stream-generate" => Ok(P::StreamGenerate),
        "generate-flow" => Ok(P::GenerateFlow),
        "classify-finite-labels" => Ok(P::ClassifyFiniteLabels),
        "extract-validated-info" => Ok(P::ExtractValidatedInfo),
        "embed-finite-vector" => Ok(P::EmbedFiniteVector),
        "interpret-sign-evidence" => Ok(P::InterpretSignEvidence),
        "present-semantic-front" => Ok(P::PresentSemanticFront),
        _ => Err("unknown semantic model profile".into()),
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Command, HostCommand, HostServiceCommand};
    use clap::Parser;

    #[test]
    fn explicit_profiles_require_selection_and_reject_ambiguous_or_unknown_input() {
        let base = [
            "conduit",
            "host",
            "service",
            "install",
            "release.json",
            "--state-dir",
            "state",
        ];
        assert!(Cli::try_parse_from(
            base.into_iter()
                .chain(["--model-profile", "stream-generate"])
        )
        .is_err());
        let mut args = base.to_vec();
        args.extend([
            "--selected-model",
            "local",
            "--model-endpoint",
            "http://127.0.0.1:11434",
            "--model-memory-mib",
            "2048",
            "--model-profile",
            "stream-generate",
        ]);
        let cli = Cli::try_parse_from(&args).unwrap();
        let Some(Command::Host {
            command:
                Some(HostCommand::Service {
                    command: HostServiceCommand::Install { model, .. },
                }),
        }) = cli.command
        else {
            panic!("installed model selection")
        };
        assert_eq!(
            model.model_profiles,
            vec![conduit_std_host::hosted_local_model::LocalModelKindProfile::StreamGenerate]
        );
        assert!(Cli::try_parse_from(args.iter().copied().chain(["--model-generation"])).is_err());
        assert!(
            Cli::try_parse_from(args.iter().copied().chain(["--model-profile", "unknown"]))
                .is_err()
        );
        let both = Cli::try_parse_from(
            args.iter()
                .copied()
                .chain(["--model-profile", "embed-finite-vector"]),
        )
        .unwrap();
        let Some(Command::Host {
            command:
                Some(HostCommand::Service {
                    command: HostServiceCommand::Install { model, .. },
                }),
        }) = both.command
        else {
            panic!("installed model profiles")
        };
        assert_eq!(model.model_profiles.len(), 2);
    }

    #[test]
    fn selected_model_generation_requires_complete_explicit_selection() {
        let base = [
            "conduit",
            "host",
            "service",
            "install",
            "release.json",
            "--state-dir",
            "state",
        ];
        assert!(Cli::try_parse_from(base.into_iter().chain(["--model-generation"])).is_err());
        let selected = [
            "--selected-model",
            "local",
            "--model-endpoint",
            "http://127.0.0.1:11434",
            "--model-memory-mib",
            "2048",
        ];
        for generation in [false, true] {
            let mut args = base.to_vec();
            args.extend(selected);
            if generation {
                args.push("--model-generation");
            }
            let cli = Cli::try_parse_from(args).unwrap();
            let Some(Command::Host {
                command:
                    Some(HostCommand::Service {
                        command: HostServiceCommand::Install { model, .. },
                    }),
            }) = cli.command
            else {
                panic!("installed model selection")
            };
            assert_eq!(model.model_generation, generation);
            assert_eq!(model.model_memory_mib, Some(2048));
        }
    }
}
