#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HeavySuite {
    Browser,
    Conduitos,
    Esp32,
}

#[derive(Debug)]
pub(super) struct CommandProofSpec {
    pub(super) id: &'static str,
    pub(super) exact_inputs: &'static [&'static str],
    pub(super) input_prefixes: &'static [&'static str],
    pub(super) workspace_packages: &'static [&'static str],
    pub(super) heavy_suites: &'static [HeavySuite],
}

impl CommandProofSpec {
    pub(super) fn owns(&self, path: &str) -> bool {
        self.exact_inputs.contains(&path)
            || self
                .input_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
    }
}

// Repository commands are registered by the proof contracts they own. An
// undeclared xtask input remains a global fallback; adding a command here is a
// reviewable claim that its effects do not cross the named suite boundary.
pub(super) const COMMAND_PROOFS: &[CommandProofSpec] = &[
    CommandProofSpec {
        id: "ci.pages-resolver",
        exact_inputs: &["tools/xtask-dispatch/src/ci_dispatch/pages_resolver.rs"],
        input_prefixes: &[],
        workspace_packages: &["conduit-xtask-dispatch"],
        heavy_suites: &[],
    },
    CommandProofSpec {
        id: "repository.plots",
        exact_inputs: &["plots/inventory.toml", "tools/xtask/src/commands/plots.rs"],
        input_prefixes: &["tools/xtask/src/commands/plots/"],
        workspace_packages: &["xtask"],
        heavy_suites: &[HeavySuite::Browser],
    },
    CommandProofSpec {
        id: "repository.esp32-make",
        exact_inputs: &["tools/xtask/src/commands/esp32_firmware.rs"],
        input_prefixes: &[],
        workspace_packages: &["xtask"],
        heavy_suites: &[HeavySuite::Esp32],
    },
    CommandProofSpec {
        id: "repository.conduitos",
        exact_inputs: &["tools/xtask/src/commands/conduitos.rs"],
        input_prefixes: &["tools/xtask/src/commands/conduitos/"],
        workspace_packages: &["xtask"],
        heavy_suites: &[HeavySuite::Conduitos],
    },
];

pub(super) fn proofs_for_path(path: &str) -> Vec<&'static CommandProofSpec> {
    COMMAND_PROOFS
        .iter()
        .filter(|spec| {
            spec.owns(path) || (spec.id == "repository.plots" && is_canonical_plot_source(path))
        })
        .collect()
}

fn is_canonical_plot_source(path: &str) -> bool {
    path.strip_prefix("plots/")
        .and_then(|path| path.strip_suffix("/main.conduit"))
        .is_some_and(|slug| !slug.is_empty() && !slug.contains('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_plots_owns_only_canonical_authored_sources_under_plots() {
        assert!(is_canonical_plot_source("plots/hello/main.conduit"));
        assert!(is_canonical_plot_source(
            "plots/not-yet-inventory/main.conduit"
        ));

        for path in [
            "plots/little-seismograph/fixture/src/lib.rs",
            "plots/little-seismograph/fixture/main.conduit",
            "plots/hello/README.md",
            "plots/main.conduit",
        ] {
            assert!(!is_canonical_plot_source(path), "{path}");
        }
    }
}
