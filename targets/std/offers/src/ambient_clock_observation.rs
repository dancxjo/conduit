//! Exact hosted Back for the standalone civil clock and weather observation.
use conduit_core::{
    resource_requirement, semantic_digest, AuthorityRequirement, Back, BackOfferBuilder,
    CapabilityOffer, HostCallRequirement,
};
use conduit_semantic_catalog::{ambient_clock_kind, AMBIENT_CLOCK_OBSERVATION_BYTES};

pub const AMBIENT_CLOCK_SOURCE_IMPLEMENTATION: &str = "std/ambient-clock-weather-worker@1";
pub const AMBIENT_CLOCK_SOURCE_CALL: &str = "conduit.host/ambient-clock-weather-observation@1";
pub const AMBIENT_CLOCK_SOURCE_AUTHORITY: &str =
    "conduit.authority/ambient-clock-weather-observation@1";
pub const AMBIENT_CLOCK_SOURCE_RESOURCE: &str = "conduit.resource/ambient-clock-weather-source@1";

pub fn ambient_clock_source_offer() -> Result<CapabilityOffer, String> {
    let kind = ambient_clock_kind()?;
    let subject = kind.kind_id.clone();
    let implementation = AMBIENT_CLOCK_SOURCE_IMPLEMENTATION;
    let digest = semantic_digest(
        implementation,
        concat!(
            include_str!("ambient_clock_observation.rs"),
            include_str!("../../src/ambient_clock.rs"),
            include_str!("../../src/ambient_clock/live.rs"),
            include_str!("../../src/ambient_clock/wire.rs"),
            include_str!("../../src/ambient_weather.rs"),
            include_str!("../../src/ambient_weather/current_wire.rs"),
            include_str!("../../src/ambient_weather/current_wire/json.rs"),
            include_str!("../../src/ambient_weather/http.rs"),
            include_str!("../../src/ambient_weather/http/connect.rs"),
            include_str!("../../src/ambient_weather/wire.rs"),
            include_str!("../../src/ambient_weather/decimal.rs"),
            include_str!("../../src/selected_native_source/worker.rs"),
            include_str!("../../src/selected_native_source/selection.rs"),
            include_str!("../../src/installed_std/body_kernel/native_source.rs"),
            include_str!("../../../../semantics/catalog/src/ambient_display.rs"),
            include_str!("../../../../plots/ambient-display/types.conduit"),
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
            capability_id: implementation.into(),
            implementation_id: implementation.into(),
            execution_profile_id: super::AMBIENT_HOME_SOURCE_PROFILE.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: AMBIENT_CLOCK_SOURCE_CALL.into(),
                target_kind: Some(subject.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
                maximum_output_bytes: AMBIENT_CLOCK_OBSERVATION_BYTES,
            }],
            resource_requirements: vec![resource_requirement(AMBIENT_CLOCK_SOURCE_RESOURCE, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AMBIENT_CLOCK_SOURCE_AUTHORITY.into(),
                host_call_contract_id: AMBIENT_CLOCK_SOURCE_CALL.into(),
                subject_kind: subject,
            }],
        },
    )
    .build())
}
