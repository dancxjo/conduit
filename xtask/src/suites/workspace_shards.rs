use crate::process::Step;

macro_rules! package_test_shard {
    ($packages:ident, $step:ident, $id:literal, $description:literal, [$($package:literal),+ $(,)?], [$($trailing:literal),* $(,)?]) => {
        const $packages: &[&str] = &[$($package),+];
        const $step: Step = Step::new(
            $id,
            $description,
            "cargo",
            &["test", $("-p", $package,)+ $($trailing,)*],
        );
    };
}

package_test_shard!(
    FOUNDATION_TEST_PACKAGES,
    FOUNDATION_TEST_STEP,
    "check.test.foundation",
    "Foundation crate unit and integration tests",
    [
        "conduit-host-avr-fabrication",
        "conduit-host-browser-fabrication",
        "conduit-host-conduitos-fabrication",
        "conduit-host-esp32-fabrication",
        "conduit-host-hosted",
        "conduit-host-orange-pi",
        "conduit-host-raspberry-pi",
        "conduit-host-rp2040",
        "conduit-linear-framebuffer-fabrication",
        "conduit-rp2040-pio-audio-extension",
        "conduit-workspace-fabrication",
        "conduit-bluetooth",
        "conduit-assigned-plan",
        "conduit-alife",
        "conduit-audio",
        "conduit-data",
        "conduit-human",
        "conduit-core",
        "conduit-create-oi",
        "conduit-mpu6050",
        "conduit-ssd1306",
        "conduit-embedded-build",
        "conduit-kernel",
        "conduit-language",
        "conduit-plan-lowering",
        "conduit-form",
        "conduit-host-fabrication",
        "conduit-planner",
        "conduit-signal",
        "conduit-signal-conformance",
        "conduit-alife-distributed-conformance",
        "conduit-r1-network-conformance",
        "conduit-semantic-catalog",
        "conduit-midi",
        "conduit-presentation",
        "conduit-robotics",
        "conduit-body",
        "conduit-body-fabrication",
        "conduit-net",
        "conduit-rp2040-network-realization",
        "conduit-wire",
        "conduit-web",
        "conduit-text",
        "conduit-time",
        "conduit-system-continuity",
        "conduit-observatory",
    ],
    []
);

package_test_shard!(
    HOST_TEST_PACKAGES,
    HOST_TEST_STEP,
    "check.test.hosts",
    "Host and fixture unit and integration tests",
    [
        "conduit-browser-host",
        "conduit-std-host",
        "conduit-std-offers",
        "conduit-browser-runtime",
        "conduitos",
        "patchbay-hosted",
        "patchbay-model",
        "patchbay-html",
        "patchbay-native",
    ],
    []
);

package_test_shard!(
    PRODUCT_TEST_PACKAGES,
    PRODUCT_TEST_STEP,
    "check.test.products",
    "Product and integration unit and integration tests",
    [
        "conduit-ai",
        "conduit-chat",
        "conduit-synth",
        "conduit-composite",
        "conduit-pete",
        "conduit-tongues",
        "patchbay-control",
        "conduit",
        "conduit-xtask-dispatch",
        "xtask",
    ],
    ["--features", "conduit-tongues/speech"]
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
    pub const ALL: [Self; 6] = [
        Self::Lint,
        Self::TestFoundation,
        Self::TestHosts,
        Self::TestProducts,
        Self::Portable,
        Self::Pico,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Lint => "lint",
            Self::TestFoundation => "test-foundation",
            Self::TestHosts => "test-hosts",
            Self::TestProducts => "test-products",
            Self::Portable => "portable",
            Self::Pico => "pico",
        }
    }

    pub const fn test_packages(self) -> &'static [&'static str] {
        match self {
            Self::TestFoundation => FOUNDATION_TEST_PACKAGES,
            Self::TestHosts => HOST_TEST_PACKAGES,
            Self::TestProducts => PRODUCT_TEST_PACKAGES,
            _ => &[],
        }
    }

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
            Self::TestProducts => false,
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
        WorkspaceShard, FOUNDATION_TEST_PACKAGES, FOUNDATION_TEST_STEP, HOST_TEST_PACKAGES,
        HOST_TEST_STEP, PRODUCT_TEST_PACKAGES, PRODUCT_TEST_STEP,
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

    #[test]
    fn every_test_shard_names_packages_with_an_explicit_package_flag() {
        for step in [&FOUNDATION_TEST_STEP, &HOST_TEST_STEP, &PRODUCT_TEST_STEP] {
            assert_eq!(step.args.first(), Some(&"test"), "{} command", step.id);
            let options = &step.args[1..];
            let package_end = options
                .iter()
                .position(|argument| *argument == "--features")
                .unwrap_or(options.len());
            let packages = &options[..package_end];
            assert_eq!(packages.len() % 2, 0, "{} package pairs", step.id);
            for pair in packages.as_chunks::<2>().0 {
                assert_eq!(pair[0], "-p", "{} package flag for {}", step.id, pair[1]);
            }
            if package_end < options.len() {
                assert_eq!(
                    &options[package_end..],
                    ["--features", "conduit-tongues/speech"],
                    "{} trailing options",
                    step.id
                );
            }
        }
    }
}
