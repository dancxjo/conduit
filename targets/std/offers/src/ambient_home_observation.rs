//! Descriptor for a separately initialized and authorized Home source.
use conduit_core::{
    resource_requirement, semantic_digest, AuthorityRequirement, Back, BackOfferBuilder,
    CapabilityOffer, HostCallRequirement,
};
use conduit_semantic_catalog::{ambient_home_observation_kind, AMBIENT_HOME_OBSERVATION_BYTES};

pub const AMBIENT_HOME_SOURCE_IMPLEMENTATION: &str = "std/resident-ambient-observation-worker@1";
pub const AMBIENT_HOME_SOURCE_PROFILE: &str = "conduit.std/linux-native-source-one-slot@1";
pub const AMBIENT_HOME_SOURCE_CALL: &str = "conduit.host/resident-ambient-observation@1";
pub const AMBIENT_HOME_SOURCE_AUTHORITY: &str = "conduit.authority/resident-ambient-observation@1";
pub const AMBIENT_HOME_SOURCE_RESOURCE: &str =
    "conduit.resource/resident-ambient-observation-source@1";

pub fn ambient_home_source_offer() -> Result<CapabilityOffer, String> {
    let kind = ambient_home_observation_kind()?;
    let implementation = AMBIENT_HOME_SOURCE_IMPLEMENTATION;
    let digest = semantic_digest(
        implementation,
        concat!(
            include_str!("ambient_home_observation.rs"),
            include_str!("../../src/ambient_home_observation.rs"),
            include_str!("../../src/ambient_clock.rs"),
            include_str!("../../src/ambient_clock/live.rs"),
            include_str!("../../src/ambient_clock/wire.rs"),
            include_str!("../../src/ambient_wallpaper.rs"),
            include_str!("../../src/ambient_wallpaper/acquisition.rs"),
            include_str!("../../src/ambient_wallpaper/selection_wire.rs"),
            include_str!("../../src/ambient_weather.rs"),
            include_str!("../../src/ambient_weather/current_wire.rs"),
            include_str!("../../src/ambient_weather/current_wire/json.rs"),
            include_str!("../../src/ambient_weather/http.rs"),
            include_str!("../../src/ambient_weather/http/connect.rs"),
            include_str!("../../src/ambient_weather/wire.rs"),
            include_str!("../../src/ambient_weather/decimal.rs"),
            include_str!("../../src/selected_native_source/worker.rs"),
            include_str!("../../../../semantics/catalog/src/ambient_home_observation.rs"),
            include_str!("../../../../plots/ambient-display/types.conduit")
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let subject = kind.kind_id.clone();
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: implementation.into(),
            implementation_id: implementation.into(),
            execution_profile_id: AMBIENT_HOME_SOURCE_PROFILE.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: AMBIENT_HOME_SOURCE_CALL.into(),
                target_kind: Some(subject.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 8,
                maximum_output_bytes: AMBIENT_HOME_OBSERVATION_BYTES,
            }],
            resource_requirements: vec![resource_requirement(AMBIENT_HOME_SOURCE_RESOURCE, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AMBIENT_HOME_SOURCE_AUTHORITY.into(),
                host_call_contract_id: AMBIENT_HOME_SOURCE_CALL.into(),
                subject_kind: subject,
            }],
        },
    )
    .build())
}
