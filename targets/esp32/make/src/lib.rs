use conduit_host_make::{
    BaseSelection, HostMakePackage, ImplementationOffer, MakeAnchor, MakeContribution,
};

pub mod descriptor;
pub mod family;
pub mod wroom32;

pub use descriptor::{
    esp32_descriptor_binding, validate_esp32_binding, validate_esp32_descriptor,
    validate_esp32_target, Esp32BoardDescriptor, Esp32DescriptorDiagnostic,
};
pub use family::{
    Esp32FamilyTarget, Esp32FamilyTargetFacts, NATIVE_SPORE_FLASH_BYTES, NATIVE_SPORE_REGION_BYTES,
    NATIVE_SPORE_REGION_START,
};
pub use wroom32::hw463_esp_wroom_32_sample;

#[cfg(test)]
mod descriptor_tests;
#[cfg(test)]
mod family_tests;
#[cfg(test)]
mod wroom32_tests;

pub struct Esp32MakePackage;

pub fn features_for_bases(bases: &[BaseSelection]) -> Result<Vec<String>, String> {
    let MakeContribution::Anchor(anchor) = Esp32MakePackage.contribution() else {
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
        target_patterns: Esp32FamilyTarget::ALL
            .into_iter()
            .map(|target| {
                let facts = target.facts();
                format!("esp32/{}/{}", facts.architecture, facts.machine)
            })
            .collect(),
        prerequisites: Vec::new(),
        build_feature: Some(feature.into()),
    }
}

impl HostMakePackage for Esp32MakePackage {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Anchor(MakeAnchor {
            package_id: "conduit-host-esp32@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: Esp32FamilyTarget::ALL
                .into_iter()
                .map(Esp32FamilyTarget::target_descriptor)
                .collect(),
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

#[cfg(test)]
fn headless_test_profile() -> conduit_host_make::HostProfile {
    use conduit_host_make::{HostBounds, HostPolicy, TargetSelection, HOST_PROFILE_SCHEMA};

    conduit_host_make::HostProfile {
        schema: HOST_PROFILE_SCHEMA.into(),
        name: "headless-test-fixture".into(),
        source_configuration_id: None,
        target: TargetSelection {
            family: "fixture".into(),
            architecture: "fixture".into(),
            machine: "fixture".into(),
            build_profile: "release".into(),
            make_descriptor: None,
        },
        host_core: "host-core/conduitos@1".into(),
        fragments: Vec::new(),
        capabilities: Vec::new(),
        host_calls: Vec::new(),
        resources: Vec::new(),
        bases: Vec::new(),
        drivers: Vec::new(),
        lines: Vec::new(),
        presenters: Vec::new(),
        facilities: Vec::new(),
        exclusions: Vec::new(),
        policy: HostPolicy {
            authority_profile: "authority/explicit@1".into(),
            trust_profile: "trust/local-explicit@1".into(),
            update_profile: "update/rebuild@1".into(),
            ambient_defaults: false,
        },
        bounds: HostBounds {
            static_memory_bytes: 8 * 1024 * 1024,
            heap_arena_bytes: 1,
            queue_items: 64,
            buffered_bytes: 16 * 1024,
            active_instances: 16,
            operation_slots: 8,
            timer_slots: 4,
            line_sessions: 1,
            evidence_items: 64,
        },
    }
}
