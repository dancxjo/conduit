use serde::{Deserialize, Serialize};

use crate::{BaseSelection, HostBounds, HostProfile};

pub const ARCHITECTURE_PACKAGE_SCHEMA: &str = "conduit.host/architecture-package@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SporeOutputKind {
    NativeBundle,
    BrowserBundle,
    Uf2,
    DiskImage,
    EfiArtifact,
    Esp32Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureBuildSelection {
    pub architecture_package_id: String,
    pub architecture_package_revision: u32,
    pub toolchain_identity: String,
    pub builder_adapter: String,
    pub deployment_adapter: Option<String>,
    pub features: Vec<String>,
    pub selected_base_implementations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitecturePackage {
    pub id: &'static str,
    pub revision: u32,
    pub target_patterns: &'static [&'static str],
    pub toolchain: &'static str,
    pub builder: &'static str,
    pub outputs: &'static [SporeOutputKind],
    pub deployment_adapter: Option<&'static str>,
    pub base_features: &'static [(&'static str, &'static str, &'static str)],
    pub maxima: HostBounds,
}

impl ArchitecturePackage {
    pub fn derive(
        &self,
        profile: &HostProfile,
        output: &SporeOutputKind,
    ) -> Result<ArchitectureBuildSelection, ArchitecturePackageDiagnostic> {
        if !self.outputs.contains(output) {
            return Err(ArchitecturePackageDiagnostic::UnsupportedOutput {
                package: self.id.into(),
                output: output.clone(),
            });
        }
        let mut features = Vec::new();
        let mut implementations = Vec::new();
        for selection in &profile.bases {
            implementations.push(selection.driver.clone());
            if let Some((_, _, feature)) = self
                .base_features
                .iter()
                .find(|(kind, driver, _)| *kind == selection.kind && *driver == selection.driver)
            {
                features.push((*feature).to_owned());
            } else if !self.base_features.is_empty() {
                return Err(ArchitecturePackageDiagnostic::UnsupportedBase {
                    package: self.id.into(),
                    kind: selection.kind.clone(),
                    implementation: selection.driver.clone(),
                });
            }
        }
        features.sort();
        features.dedup();
        implementations.sort();
        Ok(ArchitectureBuildSelection {
            architecture_package_id: self.id.into(),
            architecture_package_revision: self.revision,
            toolchain_identity: self.toolchain.into(),
            builder_adapter: self.builder.into(),
            deployment_adapter: self.deployment_adapter.map(str::to_owned),
            features,
            selected_base_implementations: implementations,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchitecturePackageDiagnostic {
    UnknownTarget {
        target: String,
    },
    UnsupportedOutput {
        package: String,
        output: SporeOutputKind,
    },
    UnsupportedBase {
        package: String,
        kind: String,
        implementation: String,
    },
}

pub fn architecture_package_for(
    profile: &HostProfile,
) -> Result<&'static ArchitecturePackage, ArchitecturePackageDiagnostic> {
    let target = profile.target.key();
    architecture_packages()
        .iter()
        .find(|package| {
            package
                .target_patterns
                .iter()
                .any(|pattern| target_matches(pattern, &target))
        })
        .ok_or(ArchitecturePackageDiagnostic::UnknownTarget { target })
}

pub fn derive_esp32_feature_closure(
    bases: &[BaseSelection],
) -> Result<Vec<String>, ArchitecturePackageDiagnostic> {
    let profile = HostProfile {
        schema: crate::HOST_PROFILE_SCHEMA.into(),
        name: "esp32-feature-projection".into(),
        source_configuration_id: Some("sha256:feature-projection".into()),
        target: crate::TargetSelection {
            family: "esp32".into(),
            architecture: "xtensa-lx6".into(),
            machine: "hw-463-esp-wroom-32".into(),
            build_profile: "release".into(),
            fabrication_descriptor: Some("observed/hw-463-esp-wroom-32@1".into()),
        },
        host_core: "host-core/conduitos@1".into(),
        fragments: Vec::new(),
        capabilities: Vec::new(),
        host_operations: Vec::new(),
        resources: Vec::new(),
        bases: bases.to_vec(),
        drivers: Vec::new(),
        lines: Vec::new(),
        presenters: Vec::new(),
        facilities: Vec::new(),
        exclusions: Vec::new(),
        policy: crate::HostPolicy {
            authority_profile: "authority/explicit@1".into(),
            trust_profile: "trust/local-explicit@1".into(),
            update_profile: "update/rebuild@1".into(),
            ambient_defaults: false,
        },
        bounds: embedded_maxima(),
    };
    architecture_package_for(&profile)?
        .derive(&profile, &SporeOutputKind::Esp32Image)
        .map(|selection| selection.features)
}

pub fn architecture_packages() -> &'static [ArchitecturePackage] {
    macro_rules! package {
        ($id:expr, $targets:expr, $toolchain:expr, $builder:expr, $outputs:expr,
         $deployment:expr, $features:expr, $maxima:expr $(,)?) => {
            package!(
                $id,
                1,
                $targets,
                $toolchain,
                $builder,
                $outputs,
                $deployment,
                $features,
                $maxima,
            )
        };
        ($id:expr, $revision:expr, $targets:expr, $toolchain:expr, $builder:expr, $outputs:expr,
         $deployment:expr, $features:expr, $maxima:expr $(,)?) => {
            ArchitecturePackage {
                id: $id,
                revision: $revision,
                target_patterns: $targets,
                toolchain: $toolchain,
                builder: $builder,
                outputs: $outputs,
                deployment_adapter: $deployment,
                base_features: $features,
                maxima: $maxima,
            }
        };
    }
    static PACKAGES: std::sync::OnceLock<Vec<ArchitecturePackage>> = std::sync::OnceLock::new();
    PACKAGES.get_or_init(|| {
        vec![
            package!(
                "hosted-native@1",
                &["std/*/*"],
                "rustc:stable",
                "conduit-host-fabrication/build-host-image@1",
                &[SporeOutputKind::NativeBundle],
                Some("conduit.deploy/native-directory@1"),
                &[
                    ("clock/monotonic", "hosted/monotonic-clock@1", "base-clock"),
                    ("serial/text", "hosted/serial@1", "base-serial"),
                    (
                        "storage/protected-file",
                        "hosted/protected-file@1",
                        "base-protected-file",
                    ),
                    ("timer/monotonic", "hosted/monotonic-clock@1", "base-timer"),
                ],
                hosted_maxima(),
            ),
            package!(
                "browser-wasm@1",
                &["browser/wasm32/page"],
                "rustc:stable+wasm32-unknown-unknown",
                "conduit-host-fabrication/build-host-image@1",
                &[SporeOutputKind::BrowserBundle],
                Some("conduit.deploy/browser-directory@1"),
                &[("browser/dom", "browser/dom@1", "base-browser-dom")],
                hosted_maxima(),
            ),
            package!(
                "pico-rp2040@1",
                &["conduitos/thumbv6m/pico-w"],
                "rustc:stable+thumbv6m-none-eabi",
                "conduit-host-fabrication/build-host-image@1",
                &[SporeOutputKind::Uf2],
                None,
                &[("serial/text", "pico/usb-cdc@1", "line-usb-cdc")],
                embedded_maxima(),
            ),
            package!(
                "esp32-firmware@1",
                2,
                &["esp32/xtensa-lx6/*"],
                "esp-rs/rust-build@v1.91.1.0",
                "esp32-firmware/architecture-package-runner@2",
                &[SporeOutputKind::Esp32Image],
                None,
                &[
                    ("kernel/signal", "esp32/kernel-signal@1", "kernel-signal"),
                    (
                        "line/bluetooth-le-gatt",
                        "esp32/bluetooth-le-gatt@1",
                        "bluetooth",
                    ),
                ],
                embedded_maxima(),
            ),
            package!(
                "conduitos-image@1",
                &["conduitos/x86_64/pc", "conduitos/aarch64/virt"],
                "rustc:stable+llvm-tools",
                "conduit-host-fabrication/build-host-image@1",
                &[SporeOutputKind::DiskImage, SporeOutputKind::EfiArtifact],
                None,
                &[],
                hosted_maxima(),
            ),
        ]
    })
}

fn target_matches(pattern: &str, target: &str) -> bool {
    let actual = target.split('/');
    pattern
        .split('/')
        .zip(actual)
        .all(|(expected, found)| expected == "*" || expected == found)
}

fn hosted_maxima() -> HostBounds {
    maxima(2 * 1024 * 1024 * 1024, 1_048_576)
}
fn embedded_maxima() -> HostBounds {
    maxima(4 * 1024 * 1024, 4096)
}
fn maxima(memory: u64, items: u32) -> HostBounds {
    HostBounds {
        static_memory_bytes: memory,
        heap_arena_bytes: memory,
        queue_items: items,
        buffered_bytes: memory,
        active_instances: items,
        operation_slots: items,
        timer_slots: items,
        line_sessions: items,
        evidence_items: items,
    }
}
