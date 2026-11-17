//! Clock-only realization of complete Home observations with explicit absence.
use conduit_core::*;
pub const AMBIENT_OBSERVE_IMPLEMENTATION: &str = "std/ambient-clock-home@1";
pub const AMBIENT_OBSERVE_PROFILE: &str = "std/ambient-clock-home-cpu@1";
pub const AMBIENT_OBSERVE_CALL: &str = "conduit.host/ambient-clock-home@1";
pub const AMBIENT_RESIDENT_OBSERVE_IMPLEMENTATION: &str = "std/resident-ambient-clock-home@1";
pub const AMBIENT_RESIDENT_OBSERVE_PROFILE: &str = "std/resident-ambient-clock-home-cpu@1";
pub const AMBIENT_RESIDENT_OBSERVE_CALL: &str = "conduit.host/resident-ambient-clock-home@1";
pub fn ambient_observe_cpu_offer() -> Result<CapabilityOffer, String> {
    observe_offer(false)
}
pub fn ambient_resident_observe_cpu_offer() -> Result<CapabilityOffer, String> {
    observe_offer(true)
}
fn observe_offer(resident: bool) -> Result<CapabilityOffer, String> {
    let kind = if resident {
        conduit_semantic_catalog::ambient_resident_observe_kind()?
    } else {
        conduit_semantic_catalog::ambient_observe_kind()?
    };
    let (implementation, profile, call) = if resident {
        (
            AMBIENT_RESIDENT_OBSERVE_IMPLEMENTATION,
            AMBIENT_RESIDENT_OBSERVE_PROFILE,
            AMBIENT_RESIDENT_OBSERVE_CALL,
        )
    } else {
        (
            AMBIENT_OBSERVE_IMPLEMENTATION,
            AMBIENT_OBSERVE_PROFILE,
            AMBIENT_OBSERVE_CALL,
        )
    };
    let digest = semantic_digest(
        implementation,
        concat!(
            include_str!("ambient_observe.rs"),
            include_str!("../../src/installed_std/ambient_observe_back.rs"),
            include_str!("../../src/ambient_clock.rs"),
            include_str!("../../src/ambient_clock/wire.rs"),
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
            execution_profile_id: profile.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: call.into(),
                target_kind: Some(kind.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: 8,
                maximum_output_bytes: 8192,
            }],
            // The complete original IMAGE receipt counts the prepared Back buffers;
            // no separate projection worker or uncharged provider is retained.
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
