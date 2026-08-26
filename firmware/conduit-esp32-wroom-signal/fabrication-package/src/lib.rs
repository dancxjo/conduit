use conduit_host_fabrication::{
    BaseSelection, FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub mod descriptor;
pub mod wroom32;

pub use descriptor::{
    esp32_descriptor_binding, validate_esp32_binding, validate_esp32_descriptor,
    validate_esp32_target, Esp32BoardDescriptor, Esp32DescriptorDiagnostic,
};
pub use wroom32::hw463_esp_wroom_32_sample;

#[cfg(test)]
mod descriptor_tests;
#[cfg(test)]
mod wroom32_tests;

pub struct Esp32FabricationPackage;

pub fn features_for_bases(bases: &[BaseSelection]) -> Result<Vec<String>, String> {
    let FabricationContribution::Anchor(anchor) = Esp32FabricationPackage.contribution() else {
        unreachable!("ESP32 is an anchor package")
    };
    let mut features = bases
        .iter()
        .map(|base| {
            anchor
                .offers
                .iter()
                .find(|offer| {
                    offer.base_kind == base.kind && offer.implementation_id == base.driver
                })
                .and_then(|offer| offer.build_feature.clone())
                .ok_or_else(|| {
                    format!(
                        "unsupported ESP32 Base implementation {} for {}",
                        base.driver, base.kind
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    features.sort();
    features.dedup();
    Ok(features)
}

fn offer(kind: &str, implementation: &str, feature: &str) -> ImplementationOffer {
    ImplementationOffer {
        base_kind: kind.into(),
        implementation_id: implementation.into(),
        implementation_revision: 1,
        target_patterns: vec!["esp32/xtensa-lx6/*".into()],
        prerequisites: Vec::new(),
        build_feature: Some(feature.into()),
    }
}

impl HostFabricationPackage for Esp32FabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        let descriptor = hw463_esp_wroom_32_sample();
        validate_esp32_descriptor(&descriptor)
            .expect("package-owned WROOM descriptor must remain valid");
        let descriptor_binding = esp32_descriptor_binding(&descriptor)
            .expect("package-owned WROOM descriptor must have an exact binding");
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduit-host-esp32@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![TargetDescriptor {
                label: "ESP32-WROOM-32".into(),
                family: "esp32".into(),
                architecture: "xtensa-lx6".into(),
                machine: "hw-463-esp-wroom-32".into(),
                board: Some("hw-463-esp-wroom-32".into()),
                os: None,
                host_core: "host-core/conduitos@1".into(),
                presenter: None,
                host_operations: Vec::new(),
                toolchain_identity: "esp-rs/rust-build@v1.91.1.0".into(),
                builder_adapter: "conduit-host-esp32/build-image@1".into(),
                deployment_adapter: None,
                outputs: vec![SporeOutputKind::Esp32Image],
                default_output: SporeOutputKind::Esp32Image,
                post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
                fabrication_descriptors: vec![descriptor_binding],
                maxima: HostBounds {
                    static_memory_bytes: 64 * 1024 * 1024,
                    heap_arena_bytes: 64 * 1024 * 1024,
                    queue_items: 4096,
                    buffered_bytes: 4 * 1024 * 1024,
                    active_instances: 4096,
                    operation_slots: 4096,
                    timer_slots: 4096,
                    line_sessions: 4096,
                    evidence_items: 4096,
                },
            }],
            offers: vec![
                offer("kernel/signal", "esp32/kernel-signal@1", "kernel-signal"),
                offer(
                    "line/bluetooth-le-gatt",
                    "esp32/bluetooth-le-gatt@1",
                    "bluetooth",
                ),
            ],
        })
    }
}
