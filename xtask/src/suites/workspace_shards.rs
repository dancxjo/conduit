use crate::process::Step;

#[cfg(test)]
const FOUNDATION_TEST_PACKAGES: &[&str] = &[
    "conduit-bluetooth",
    "conduit-alife",
    "conduit-core",
    "conduit-create-oi",
    "conduit-mpu6050",
    "conduit-ssd1306",
    "conduit-embedded-build",
    "conduit-kernel",
    "conduit-runtime",
    "conduit-form",
    "conduit-host-fabrication",
    "conduit-planner",
    "conduit-signal",
    "conduit-std-catalog",
    "conduit-midi",
    "conduit-presentation",
    "conduit-body",
    "conduit-net",
    "conduit-wire",
    "conduit-system-continuity",
    "conduit-observatory",
];

#[cfg(test)]
const HOST_TEST_PACKAGES: &[&str] = &[
    "conduit-host-browser-fabrication",
    "conduit-host-conduitos-fabrication",
    "conduit-host-esp32-fabrication",
    "conduit-host-hosted",
    "conduit-host-raspberry-pi",
    "conduit-host-rp2040",
    "conduit-linear-framebuffer-fabrication",
    "conduit-rp2040-pio-audio-extension",
    "conduit-workspace-fabrication",
    "conduit-browser-sim",
    "conduit-browser-host",
    "conduit-pico-sim",
    "conduit-std-host",
    "conduit-browser-runtime",
    "conduitos",
    "patchbay-model",
    "patchbay-html",
    "patchbay-native",
];

#[cfg(test)]
const PRODUCT_TEST_PACKAGES: &[&str] = &[
    "conduit-ai",
    "conduit-chat",
    "conduit-synth",
    "conduit-composite",
    "conduit-pete",
    "conduit-tongues",
    "conduit",
    "xtask",
];

const FOUNDATION_TEST_STEP: Step = Step::new(
    "check.test.foundation",
    "Foundation crate unit and integration tests",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-host-browser-fabrication",
        "-p",
        "conduit-host-conduitos-fabrication",
        "-p",
        "conduit-host-esp32-fabrication",
        "-p",
        "conduit-host-hosted",
        "-p",
        "conduit-host-raspberry-pi",
        "-p",
        "conduit-host-rp2040",
        "-p",
        "conduit-linear-framebuffer-fabrication",
        "-p",
        "conduit-rp2040-pio-audio-extension",
        "-p",
        "conduit-workspace-fabrication",
        "-p",
        "conduit-bluetooth",
        "conduit-alife",
        "-p",
        "conduit-core",
        "-p",
        "conduit-create-oi",
        "-p",
        "conduit-mpu6050",
        "-p",
        "conduit-ssd1306",
        "-p",
        "conduit-embedded-build",
        "-p",
        "conduit-kernel",
        "-p",
        "conduit-runtime",
        "-p",
        "conduit-form",
        "-p",
        "conduit-host-fabrication",
        "-p",
        "conduit-planner",
        "-p",
        "conduit-signal",
        "-p",
        "conduit-std-catalog",
        "-p",
        "conduit-midi",
        "-p",
        "conduit-presentation",
        "-p",
        "conduit-body",
        "-p",
        "conduit-net",
        "-p",
        "conduit-wire",
        "-p",
        "conduit-system-continuity",
        "-p",
        "conduit-observatory",
    ],
);

const HOST_TEST_STEP: Step = Step::new(
    "check.test.hosts",
    "Host and fixture unit and integration tests",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-browser-sim",
        "-p",
        "conduit-browser-host",
        "-p",
        "conduit-pico-sim",
        "-p",
        "conduit-std-host",
        "-p",
        "conduit-browser-runtime",
        "-p",
        "conduitos",
        "-p",
        "patchbay-model",
        "-p",
        "patchbay-html",
        "-p",
        "patchbay-native",
    ],
);

const PRODUCT_TEST_STEP: Step = Step::new(
    "check.test.products",
    "Product and integration unit and integration tests",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-ai",
        "-p",
        "conduit-chat",
        "-p",
        "conduit-synth",
        "-p",
        "conduit-composite",
        "-p",
        "conduit-pete",
        "-p",
        "conduit-tongues",
        "-p",
        "conduit",
        "-p",
        "xtask",
        "--features",
        "conduit-tongues/speech",
    ],
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceShard {
    Lint,
    TestFoundation,
    TestHosts,
    TestProducts,
    Portable,
    Pico,
}

impl WorkspaceShard {
    #[cfg(test)]
    pub const ALL: [Self; 6] = [
        Self::Lint,
        Self::TestFoundation,
        Self::TestHosts,
        Self::TestProducts,
        Self::Portable,
        Self::Pico,
    ];

    pub fn package_test_step(self) -> Option<&'static Step> {
        match self {
            Self::TestFoundation => Some(&FOUNDATION_TEST_STEP),
            Self::TestHosts => Some(&HOST_TEST_STEP),
            Self::TestProducts => Some(&PRODUCT_TEST_STEP),
            _ => None,
        }
    }

    pub fn owns(self, step: &Step) -> bool {
        match self {
            Self::Lint => matches!(step.id, "check.fmt" | "check.clippy"),
            Self::TestFoundation => {
                matches!(step.id, "check.kernel-alloc" | "check.system-continuity")
            }
            Self::TestHosts => false,
            Self::TestProducts => matches!(step.id, "check.observatory-fixture"),
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
    use std::{collections::BTreeSet, process::Command};

    use super::{
        WorkspaceShard, FOUNDATION_TEST_PACKAGES, HOST_TEST_PACKAGES, PRODUCT_TEST_PACKAGES,
    };
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
            if step.id == "check.test" {
                continue;
            }
            let owners = WorkspaceShard::ALL
                .into_iter()
                .filter(|shard| shard.owns(step))
                .count();
            assert_eq!(owners, 1, "{} must have exactly one shard", step.id);
        }
    }

    #[test]
    fn package_test_shards_cover_the_exact_workspace_once() {
        let output = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()
            .expect("cargo metadata must launch");
        assert!(output.status.success(), "cargo metadata must succeed");
        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("cargo metadata must be JSON");
        let member_ids: BTreeSet<_> = metadata["workspace_members"]
            .as_array()
            .expect("workspace_members must be an array")
            .iter()
            .map(|member| member.as_str().expect("workspace member must be a string"))
            .collect();
        let members: BTreeSet<_> = metadata["packages"]
            .as_array()
            .expect("packages must be an array")
            .iter()
            .filter(|package| {
                member_ids.contains(package["id"].as_str().expect("package id must be a string"))
            })
            .map(|package| {
                package["name"]
                    .as_str()
                    .expect("package name must be a string")
            })
            .collect();
        let assigned: Vec<_> = FOUNDATION_TEST_PACKAGES
            .iter()
            .chain(HOST_TEST_PACKAGES)
            .chain(PRODUCT_TEST_PACKAGES)
            .copied()
            .collect();
        let unique: BTreeSet<_> = assigned.iter().copied().collect();

        assert_eq!(assigned.len(), unique.len(), "test package assigned twice");
        assert_eq!(
            unique, members,
            "test shards must cover the exact workspace"
        );
    }
}
