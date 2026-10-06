use crate::process::Step;

macro_rules! package_test_shard {
    ($packages:ident, $step:ident, $id:literal, $description:literal, [$($package:literal),+ $(,)?], [$($trailing:literal),* $(,)?]) => {
        #[cfg(test)]
        const $packages: &[&str] = &[$($package),+];
        const $step: Step = Step::new(
            $id,
            $description,
            "cargo",
            &["test", "--no-fail-fast", $("-p", $package,)+ $($trailing,)*],
        );
    };
}

package_test_shard!(
    FOUNDATION_TEST_PACKAGES,
    FOUNDATION_TEST_STEP,
    "check.test.foundation",
    "Foundation crate unit and integration tests",
    [
        "conduit-host-avr-make",
        "conduit-host-browser-make",
        "conduit-host-conduitos-make",
        "conduit-host-esp32-make",
        "conduit-host-hosted",
        "conduit-host-orange-pi",
        "conduit-host-raspberry-pi",
        "conduit-host-rp2040",
        "conduit-emergency-keyword-spotter",
        "conduit-linear-framebuffer-make",
        "conduit-rp2040-pio-audio-extension",
        "conduit-workspace-make",
        "conduit-bluetooth",
        "conduit-assigned-plan",
        "conduit-alife",
        "conduit-audio",
        "conduit-data",
        "conduit-education",
        "conduit-finance",
        "conduit-human",
        "conduit-core",
        "conduit-create-oi",
        "conduit-mpu6050",
        "conduit-ssd1306",
        "conduit-embedded-build",
        "conduit-kernel",
        "conduit-language",
        "conduit-plan-lowering",
        "conduit-plot",
        "conduit-host-make",
        "conduit-planner",
        "conduit-signal",
        "conduit-speech",
        "conduit-signal-conformance",
        "conduit-alife-distributed-conformance",
        "conduit-r1-network-conformance",
        "conduit-patchbay-workbench-conformance",
        "conduit-semantic-catalog",
        "conduit-midi",
        "conduit-presentation",
        "conduit-protected-line",
        "conduit-process",
        "conduit-purpose",
        "conduit-robotics",
        "conduit-body",
        "conduit-birth-plot",
        "conduit-body-make",
        "conduit-net",
        "conduit-rp2040-network-realization",
        "conduit-wire",
        "conduit-web",
        "conduit-text",
        "conduit-time",
        "conduit-system-continuity",
        "conduit-observatory",
        "patchbay-control",
        "patchbay-graph",
        "patchbay-application",
        "patchbay-svg-mask",
    ],
    ["--features", "conduit-speech/semantic-bindings"]
);

// Host tests launch real local providers and device-discovery subprocesses.
// Keep cases within each test binary isolated; Cargo compilation and target
// jobs remain parallel.
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
        "conduit-browser-mask-offer",
        "conduit-conduitos-mask-offer",
        "conduit-little-seismograph-fixture",
        "conduitos",
        "patchbay-hosted",
        "conduit-patchbay-workbench",
        "patchbay-workbench-host-contract",
        "conduit-browser-patchbay-workbench",
    ],
    ["--", "--test-threads=1"]
);

// Separate runners isolate these host proof surfaces while each binary remains serial.
package_test_shard!(
    HOST_STD_TEST_PACKAGES,
    HOST_STD_TEST_STEP,
    "check.test.hosts-std",
    "Std host unit and integration tests",
    ["conduit-std-host", "conduit-std-offers",],
    ["--", "--test-threads=1"]
);

package_test_shard!(
    HOST_BROWSER_TEST_PACKAGES,
    HOST_BROWSER_TEST_STEP,
    "check.test.hosts-browser",
    "Browser host unit and integration tests",
    [
        "conduit-browser-host",
        "conduit-browser-runtime",
        "conduit-browser-mask-offer",
    ],
    ["--", "--test-threads=1"]
);

package_test_shard!(
    HOST_CONDUITOS_TEST_PACKAGES,
    HOST_CONDUITOS_TEST_STEP,
    "check.test.hosts-conduitos",
    "Conduitos host unit and integration tests",
    ["conduit-conduitos-mask-offer",],
    ["--", "--test-threads=1"]
);

package_test_shard!(
    HOST_WORKBENCH_TEST_PACKAGES,
    HOST_WORKBENCH_TEST_STEP,
    "check.test.hosts-workbench",
    "Workbench host unit and integration tests",
    [
        "conduit-little-seismograph-fixture",
        "patchbay-hosted",
        "conduit-patchbay-workbench",
        "patchbay-workbench-host-contract",
        "conduit-browser-patchbay-workbench",
    ],
    ["--", "--test-threads=1"]
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
        "conduit-pete-workload-conformance",
        "conduit-tongues",
        "conduit-home-model",
        "conduit-body-invitation-plot",
        "conduit-body-lifecycle-conformance",
        "conduit-tour-model",
        "conduit-tutorial-plot",
        "conduit-plot-library",
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
    TestHostsStd,
    TestHostsBrowser,
    TestHostsConduitos,
    TestHostsWorkbench,
    TestProducts,
    Portable,
    Pico,
}

impl WorkspaceShard {
    #[cfg(test)]
    pub const ALL: [Self; 9] = [
        Self::Lint,
        Self::TestFoundation,
        Self::TestHostsStd,
        Self::TestHostsBrowser,
        Self::TestHostsConduitos,
        Self::TestHostsWorkbench,
        Self::TestProducts,
        Self::Portable,
        Self::Pico,
    ];

    pub fn package_test_step(self) -> Option<&'static Step> {
        match self {
            Self::TestFoundation => Some(&FOUNDATION_TEST_STEP),
            Self::TestHosts => Some(&HOST_TEST_STEP),
            Self::TestHostsStd => Some(&HOST_STD_TEST_STEP),
            Self::TestHostsBrowser => Some(&HOST_BROWSER_TEST_STEP),
            Self::TestHostsConduitos => Some(&HOST_CONDUITOS_TEST_STEP),
            Self::TestHostsWorkbench => Some(&HOST_WORKBENCH_TEST_STEP),
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
            Self::TestHosts
            | Self::TestHostsStd
            | Self::TestHostsBrowser
            | Self::TestHostsConduitos
            | Self::TestHostsWorkbench => false,
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

    use super::*;
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
            .chain(HOST_STD_TEST_PACKAGES)
            .chain(HOST_BROWSER_TEST_PACKAGES)
            .chain(HOST_CONDUITOS_TEST_PACKAGES)
            .chain(HOST_WORKBENCH_TEST_PACKAGES)
            .chain([&crate::suites::host_protocol_shards::PACKAGE])
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
    fn host_groups_preserve_the_legacy_host_suite() {
        let grouped: BTreeSet<_> = HOST_STD_TEST_PACKAGES
            .iter()
            .chain(HOST_BROWSER_TEST_PACKAGES)
            .chain(HOST_CONDUITOS_TEST_PACKAGES)
            .chain(HOST_WORKBENCH_TEST_PACKAGES)
            .chain([&crate::suites::host_protocol_shards::PACKAGE])
            .copied()
            .collect();
        assert_eq!(grouped, HOST_TEST_PACKAGES.iter().copied().collect());
    }

    #[test]
    fn every_test_shard_names_packages_with_an_explicit_package_flag() {
        for step in WorkspaceShard::ALL
            .into_iter()
            .filter_map(WorkspaceShard::package_test_step)
        {
            assert_eq!(step.args.first(), Some(&"test"), "{} command", step.id);
            assert_eq!(
                step.args[1], "--no-fail-fast",
                "{} failure coverage",
                step.id
            );
            let options = &step.args[2..];
            let package_end = options
                .iter()
                .position(|argument| matches!(*argument, "--features" | "--"))
                .unwrap_or(options.len());
            let packages = &options[..package_end];
            assert_eq!(packages.len() % 2, 0, "{} package pairs", step.id);
            for pair in packages.as_chunks::<2>().0 {
                assert_eq!(pair[0], "-p", "{} package flag for {}", step.id, pair[1]);
            }
            if step.id.starts_with("check.test.hosts") {
                assert_eq!(
                    &options[package_end..],
                    ["--", "--test-threads=1"],
                    "{} serial process fixtures",
                    step.id
                );
            } else if package_end < options.len() {
                let ["--features", features] = &options[package_end..] else {
                    panic!("{} has unsupported trailing options", step.id);
                };
                for feature in features.split(',') {
                    let (package, feature) = feature
                        .split_once('/')
                        .expect("feature names its owning package");
                    assert!(!feature.is_empty());
                    assert!(
                        packages
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .any(|pair| pair[1] == package),
                        "{} enables a feature outside its package shard",
                        step.id
                    );
                }
            }
        }
    }
}
