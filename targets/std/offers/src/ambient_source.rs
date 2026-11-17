//! Exact descriptor for the independently authorized native ambient IO owner.
use conduit_core::*;
pub const AMBIENT_SOURCE_BASE_KIND: &str = "ambient/observation";
pub const AMBIENT_SOURCE_IMPLEMENTATION: &str = "std/ambient-observation-worker@1";
pub const AMBIENT_SOURCE_PROFILE: &str = "conduit.std/linux-native-source-one-slot@1";
pub const AMBIENT_SOURCE_CALL: &str = "conduit.host/ambient-observation@1";
pub const AMBIENT_SOURCE_AUTHORITY: &str = "conduit.authority/ambient-observation@1";
pub const AMBIENT_SOURCE_RESOURCE: &str = "conduit.resource/ambient-observation-source@1";
pub const AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES: u32 = 56 * 1024 * 1024;
/// Existing one-slot profile: maximum stack mapping + control + Tick + Home.
pub const AMBIENT_SOURCE_WORKER_BACKING_BYTES: u32 =
    2 * 1024 * 1024 + 64 * 1024 + 64 * 1024 + 8 + 8192;

pub const AMBIENT_RESIDENT_SOURCE_RESOURCE: &str =
    "conduit.resource/resident-ambient-observation-source@1";
pub const AMBIENT_RESIDENT_SOURCE_BASE_KIND: &str = "ambient/resident-observation";
pub const AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION: &str =
    "std/resident-ambient-observation-worker@1";
pub const AMBIENT_RESIDENT_SOURCE_CALL: &str = "conduit.host/resident-ambient-observation@1";
pub const AMBIENT_RESIDENT_SOURCE_AUTHORITY: &str =
    "conduit.authority/resident-ambient-observation@1";
pub fn ambient_source_offer() -> Result<CapabilityOffer, String> {
    source_offer(false)
}
pub fn ambient_resident_source_offer() -> Result<CapabilityOffer, String> {
    source_offer(true)
}
fn source_offer(resident: bool) -> Result<CapabilityOffer, String> {
    let (implementation, call, authority) = if resident {
        (
            AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION,
            AMBIENT_RESIDENT_SOURCE_CALL,
            AMBIENT_RESIDENT_SOURCE_AUTHORITY,
        )
    } else {
        (
            AMBIENT_SOURCE_IMPLEMENTATION,
            AMBIENT_SOURCE_CALL,
            AMBIENT_SOURCE_AUTHORITY,
        )
    };
    let resource = if resident {
        AMBIENT_RESIDENT_SOURCE_RESOURCE
    } else {
        AMBIENT_SOURCE_RESOURCE
    };
    let kind = if resident {
        conduit_semantic_catalog::ambient_resident_observe_kind()?
    } else {
        conduit_semantic_catalog::ambient_observe_kind()?
    };
    let digest = semantic_digest(
        implementation,
        concat!(
            include_str!("ambient_source.rs"),
            include_str!("../../src/ambient_observation.rs"),
            include_str!("../../src/ambient_clock.rs"),
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
            include_str!("../../../../plots/ambient-display/model/src/observation_source.rs"),
            include_str!("../../../../plots/ambient-display/model/src/resident.rs"),
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
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: implementation.into(),
            implementation_id: implementation.into(),
            execution_profile_id: AMBIENT_SOURCE_PROFILE.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: call.into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 8,
                maximum_output_bytes: 8192,
            }],
            resource_requirements: vec![resource_requirement(resource, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: authority.into(),
                host_call_contract_id: call.into(),
                subject_kind: kind.kind_id,
            }],
        },
    )
    .build())
}
