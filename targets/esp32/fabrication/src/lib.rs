use conduit_host_fabrication::{
    BaseSelection, FabricationAnchor, FabricationContribution, HostFabricationPackage,
    ImplementationOffer,
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

impl HostFabricationPackage for Esp32FabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
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
// CI fixture: select the independent standalone-lock prerequisite.
