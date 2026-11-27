//! Exact environment-recording JSON capture/conversion before one native worker.
//! Read custody and original producer context are independently supplied by Root;
//! neither a successful conversion nor this offer establishes durable acceptance.
use conduit_core::{
    resource_requirement, semantic_digest, AuthorityRequirement, Back, BackOfferBuilder,
    CapabilityOffer, HostCallRequirement,
};
pub const ENVIRONMENT_RECORDING_HISTORY_BASE: &str = "data/environment-recording-history";
pub const ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION: &str =
    "std/environment-recording-history-worker@1";
pub const ENVIRONMENT_RECORDING_HISTORY_CALL: &str = "conduit.host/environment-recording-history@1";
pub const ENVIRONMENT_RECORDING_HISTORY_AUTHORITY: &str =
    "conduit.authority/environment-recording-history@1";
pub const ENVIRONMENT_RECORDING_HISTORY_RESOURCE: &str =
    "conduit.resource/environment-recording-history@1";

pub fn environment_recording_history_offer() -> Result<CapabilityOffer, String> {
    let kind = conduit_semantic_catalog::environment_recording_history_kind()?;
    let subject = kind.kind_id.clone();
    let maximum = conduit_semantic_catalog::reported_generation_maximum_bytes()?;
    let digest = semantic_digest(
        ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION,
        concat!(
            include_str!("environment_recording_history.rs"),
            include_str!("../../src/environment_recording_history/mod.rs"),
            include_str!("../../src/environment_recording_history/capture.rs"),
            include_str!("../../src/environment_recording_history/configuration.rs"),
            include_str!("../../src/environment_recording_history/freeze.rs"),
            include_str!("../../src/environment_recording_history/selection.rs"),
            include_str!("../../src/environment_recording_history/connection_limits.rs"),
            include_str!("../../src/image_initialization/environment_recording_history.rs"),
            include_str!("../../src/selected_native_source/image_worker.rs"),
            include_str!("../../src/selected_native_source/worker.rs"),
            include_str!("../../src/selected_native_source/selection.rs"),
            include_str!("../../src/installed_std/body_kernel/native_source.rs"),
            include_str!("../../resource-snapshot/src/snapshot.rs"),
            include_str!("../../resource-snapshot/src/directory.rs"),
            include_str!("../../../../mechanisms/io/resource-snapshot/src/record.rs"),
            include_str!("../../../../mechanisms/io/resource-snapshot/src/record/selected.rs"),
            include_str!("../../../../semantics/catalog/src/environment_recording_history.rs"),
            include_str!("../../../../mechanisms/devices/dht11/Cargo.toml"),
            include_str!("../../../../mechanisms/devices/dht11/build.rs"),
            include_str!("../../../../mechanisms/devices/dht11/build_recording.rs"),
            include_str!("../../../../mechanisms/devices/dht11/recording.conduit"),
            include_str!("../../../../mechanisms/devices/dht11/src/lib.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/packed.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording/reader.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording/history.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording/snapshot.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation/schema.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation/wire.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation/backing.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording_backing.rs"),
            include_str!("../../../../plots/device-protocols/dht11.conduit"),
            include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION.into(),
            implementation_id: ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION.into(),
            execution_profile_id: super::AMBIENT_HOME_SOURCE_PROFILE.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: ENVIRONMENT_RECORDING_HISTORY_CALL.into(),
                target_kind: Some(subject.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
                maximum_output_bytes: maximum,
            }],
            resource_requirements: vec![resource_requirement(
                ENVIRONMENT_RECORDING_HISTORY_RESOURCE,
                1,
            )],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: ENVIRONMENT_RECORDING_HISTORY_AUTHORITY.into(),
                host_call_contract_id: ENVIRONMENT_RECORDING_HISTORY_CALL.into(),
                subject_kind: subject,
            }],
        },
    )
    .build())
}
