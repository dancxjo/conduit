use conduit_host_make::{
    HostBounds, HostMakePackage, ImplementationOffer, MakeAnchor, MakeContribution,
    PackageCatalogContribution, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub use conduit_mcu_source_build::fore_carrier;

mod bootstrap_contract;
mod gpio_initialization;
mod hardware_initialization;
pub use gpio_initialization::*;
mod membership_seed;
mod native_offer;
mod resident_protocol;
mod protocol_configuration;
pub use protocol_configuration::CheckedProtocolConfigurationImages;
pub use native_offer::{checked_i2c_offer_definition, checked_i2c_offer_image};
mod resident_work;
pub use bootstrap_contract::*;
pub use hardware_initialization::*;
pub use membership_seed::*;
pub use resident_protocol::{
    checked_dht11_resident_images, checked_dht11_resident_library, checked_ht16k33_resident_library,
};
pub use resident_work::CheckedResidentWorkPool;

pub const RECORD_SERVICE_IMPLEMENTATION: &str = "rp2040/nor-record-service@1";
pub const RECORDING_IMPLEMENTATION: &str = "rp2040/nor-record-append@1";
pub use conduit_mcu_source_build::recording::{
    CheckedRecordServiceConstruction, CheckedRecordingConstruction,
};
pub const CLOCK_IMPLEMENTATION: &str = "rp2040/monotonic-clock@1";
pub use conduit_mcu_source_build::clock::CheckedClockConstruction;
pub const RENDERER_IMPLEMENTATION: &str = "rp2040/seven-segment-renderer@1";
pub use conduit_mcu_source_build::renderer::{
    CheckedRendererConstruction, TemperatureUnit as RendererTemperatureUnit,
};
pub const PACKAGE_ID: &str = "conduit-host-rp2040@1";
pub const TARGET_ID: &str = "conduitos/thumbv6m/pico-w";
pub const BUILDER_ADAPTER: &str = "conduit-host-rp2040/build-uf2@1";

pub struct Rp2040MakePackage;

impl HostMakePackage for Rp2040MakePackage {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Anchor(MakeAnchor {
            package_id: PACKAGE_ID.into(),
            package_revision: 1,
            catalog: PackageCatalogContribution {
                initialization_contracts: std::collections::BTreeMap::from([
                    (
                        hardware_initialization::FORE_CARRIER_IMPLEMENTATION.into(),
                        conduit_mcu_source_build::fore_carrier::initialization_contract(),
                    ),
                    (
                        RECORD_SERVICE_IMPLEMENTATION.into(),
                        conduit_mcu_source_build::recording::service_initialization_contract(),
                    ),
                    (
                        RECORDING_IMPLEMENTATION.into(),
                        conduit_mcu_source_build::recording::initialization_contract(),
                    ),
                    (
                        CLOCK_IMPLEMENTATION.into(),
                        conduit_mcu_source_build::clock::initialization_contract(),
                    ),
                    (
                        WIFI_IMPLEMENTATION.into(),
                        retained_wifi_initialization_contract(),
                    ),
                    (
                        RENDERER_IMPLEMENTATION.into(),
                        conduit_mcu_source_build::renderer::initialization_contract(),
                    ),
                    (I2C_IMPLEMENTATION.into(), i2c_initialization_contract()),
                    (DHT11_IMPLEMENTATION.into(), dht11_initialization_contract()),
                    (
                        TM1637_IMPLEMENTATION.into(),
                        tm1637_initialization_contract(),
                    ),
                    (BOSCH_IMPLEMENTATION.into(), bosch_initialization_contract()),
                    (
                        HT16K33_IMPLEMENTATION.into(),
                        ht16k33_initialization_contract(),
                    ),
                ]),
                ..Default::default()
            },
            targets: vec![TargetDescriptor {
                label: "Pico W".into(),
                family: "conduitos".into(),
                architecture: "thumbv6m".into(),
                machine: "pico-w".into(),
                board: Some("pico-w".into()),
                os: None,
                host_core: "host-core/conduitos@1".into(),
                presenter: None,
                host_calls: Vec::new(),
                toolchain_identity: "rustc:1.98.1+thumbv6m-none-eabi".into(),
                builder_adapter: BUILDER_ADAPTER.into(),
                strategy: conduit_host_make::MakeStrategy::DeterministicSpecializedBuild,
                deployment_adapter: Some("conduit-host-rp2040/flash-uf2@1".into()),
                outputs: vec![SporeOutputKind::Uf2],
                default_output: SporeOutputKind::Uf2,
                post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
                make_descriptors: Vec::new(),
                maxima: HostBounds {
                    static_memory_bytes: 256 * 1024,
                    heap_arena_bytes: 256 * 1024,
                    queue_items: 4096,
                    buffered_bytes: 4 * 1024 * 1024,
                    active_instances: 4096,
                    operation_slots: 4096,
                    timer_slots: 4096,
                    line_sessions: 4096,
                    evidence_items: 4096,
                },
            }],
            offers: vec![ImplementationOffer {
                base_kind: "serial/text".into(),
                implementation_id: "pico/usb-cdc@1".into(),
                implementation_revision: 1,
                target_patterns: vec![TARGET_ID.into()],
                prerequisites: Vec::new(),
                build_feature: Some("line-usb-cdc".into()),
            }]
            .into_iter()
            .chain(hardware_initialization::offers())
            .collect(),
        })
    }
}

#[cfg(test)]
mod member_return_wire_tests;

/// Shared explicit checked Source closure for ordinary firmware fabrication.
pub use conduit_mcu_source_build::workset;

pub use conduit_mcu_source_build as source_build;
