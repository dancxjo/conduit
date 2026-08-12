use crate::process::Step;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceShard {
    Lint,
    Test,
    Portable,
    Pico,
}

impl WorkspaceShard {
    #[cfg(test)]
    pub const ALL: [Self; 4] = [Self::Lint, Self::Test, Self::Portable, Self::Pico];

    pub fn owns(self, step: &Step) -> bool {
        match self {
            Self::Lint => matches!(step.id, "check.fmt" | "check.clippy"),
            Self::Test => matches!(
                step.id,
                "check.test"
                    | "check.kernel-alloc"
                    | "check.observatory-fixture"
                    | "check.system-continuity"
            ),
            Self::Portable => {
                step.id.starts_with("check.no-std.")
                    || (step.id.starts_with("check.thumb.")
                        && !step.id.starts_with("check.thumb.firmware"))
                    || step.id.starts_with("check.wasm.")
            }
            Self::Pico => {
                step.id.starts_with("check.thumb.firmware") || step.id.ends_with(".dry-run")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceShard;
    use crate::suites::{
        check::WORKSPACE_STEPS, network_capability::NETWORK_CAPABILITY_STEPS,
        pico_compositions::PICO_COMPOSITION_STEPS,
    };

    #[test]
    fn every_workspace_gate_step_belongs_to_exactly_one_shard() {
        for step in WORKSPACE_STEPS
            .iter()
            .chain(NETWORK_CAPABILITY_STEPS)
            .chain(PICO_COMPOSITION_STEPS)
        {
            let owners = WorkspaceShard::ALL
                .into_iter()
                .filter(|shard| shard.owns(step))
                .count();
            assert_eq!(owners, 1, "{} must have exactly one shard", step.id);
        }
    }
}
