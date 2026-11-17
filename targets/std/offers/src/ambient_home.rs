//! Exact two-call Home realization descriptor; workspace is separately admitted.
use conduit_core::*;

pub const AMBIENT_HOME_IMPLEMENTATION: &str = "std/ambient-home@1";
pub const AMBIENT_HOME_PROFILE: &str = "std/ambient-home-hosted@1";
pub const AMBIENT_HOME_PROJECT_CALL: &str = "conduit.host/ambient-home-project@1";
pub const AMBIENT_HOME_CUSTODY_CALL: &str = "conduit.host/ambient-home-publish-custody@1";

pub use conduit_semantic_catalog::AmbientDisplayAspect;
pub fn ambient_aspect_identity(
    aspect: AmbientDisplayAspect,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match aspect {
        AmbientDisplayAspect::Home => (
            AMBIENT_HOME_IMPLEMENTATION,
            AMBIENT_HOME_PROFILE,
            AMBIENT_HOME_PROJECT_CALL,
            AMBIENT_HOME_CUSTODY_CALL,
        ),
        AmbientDisplayAspect::Wallpaper => (
            "std/ambient-wallpaper@1",
            "std/ambient-wallpaper-hosted@1",
            "conduit.host/ambient-wallpaper-project@1",
            "conduit.host/ambient-wallpaper-publish-custody@1",
        ),
        AmbientDisplayAspect::ClockWeather => (
            "std/ambient-clock-weather@1",
            "std/ambient-clock-weather-hosted@1",
            "conduit.host/ambient-clock-weather-project@1",
            "conduit.host/ambient-clock-weather-publish-custody@1",
        ),
    }
}
pub fn ambient_resident_aspect_identity(
    aspect: AmbientDisplayAspect,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match aspect {
        AmbientDisplayAspect::Wallpaper => (
            "std/resident-ambient-wallpaper@1",
            "std/resident-ambient-wallpaper-hosted@1",
            "conduit.host/resident-ambient-wallpaper-project@1",
            "conduit.host/resident-ambient-wallpaper-publish-custody@1",
        ),
        AmbientDisplayAspect::ClockWeather => (
            "std/resident-ambient-clock-weather@1",
            "std/resident-ambient-clock-weather-hosted@1",
            "conduit.host/resident-ambient-clock-weather-project@1",
            "conduit.host/resident-ambient-clock-weather-publish-custody@1",
        ),
        AmbientDisplayAspect::Home => ambient_aspect_identity(aspect),
    }
}
pub fn ambient_contribution_profile(kind: &str) -> Option<(AmbientDisplayAspect, bool)> {
    [
        AmbientDisplayAspect::Home,
        AmbientDisplayAspect::Wallpaper,
        AmbientDisplayAspect::ClockWeather,
    ]
    .into_iter()
    .find_map(|a| {
        if a.kind() == kind {
            Some((a, false))
        } else if conduit_semantic_catalog::ambient_resident_aspect_name(a) == Some(kind) {
            Some((a, true))
        } else {
            None
        }
    })
}
pub fn ambient_aspect_for_implementation(implementation: &str) -> Option<AmbientDisplayAspect> {
    [
        AmbientDisplayAspect::Home,
        AmbientDisplayAspect::Wallpaper,
        AmbientDisplayAspect::ClockWeather,
    ]
    .into_iter()
    .find(|a| {
        ambient_aspect_identity(*a).0 == implementation
            || ambient_resident_aspect_identity(*a).0 == implementation
    })
}
pub fn ambient_aspect_call(contract: &str) -> bool {
    [
        AmbientDisplayAspect::Home,
        AmbientDisplayAspect::Wallpaper,
        AmbientDisplayAspect::ClockWeather,
    ]
    .into_iter()
    .any(|a| {
        let (_, _, project, custody) = ambient_aspect_identity(a);
        contract == project || contract == custody || {
            let (_, _, project, custody) = ambient_resident_aspect_identity(a);
            contract == project || contract == custody
        }
    })
}
pub fn ambient_home_std_offer(workspace_bytes: u32) -> Result<CapabilityOffer, String> {
    ambient_aspect_std_offer(AmbientDisplayAspect::Home, workspace_bytes)
}
pub fn ambient_aspect_std_offer(
    aspect: AmbientDisplayAspect,
    workspace_bytes: u32,
) -> Result<CapabilityOffer, String> {
    ambient_contribution_offer(aspect, false, workspace_bytes)
}
pub fn ambient_resident_aspect_std_offer(
    aspect: AmbientDisplayAspect,
    workspace_bytes: u32,
) -> Result<CapabilityOffer, String> {
    ambient_contribution_offer(aspect, true, workspace_bytes)
}
pub fn ambient_resident_aspect_cpu_offer(
    aspect: AmbientDisplayAspect,
) -> Result<CapabilityOffer, String> {
    ambient_contribution_offer(aspect, true, ambient_home_workspace_bytes()?)
}
fn ambient_contribution_offer(
    aspect: AmbientDisplayAspect,
    resident: bool,
    workspace_bytes: u32,
) -> Result<CapabilityOffer, String> {
    let (implementation, profile, project, custody) = if resident {
        ambient_resident_aspect_identity(aspect)
    } else {
        ambient_aspect_identity(aspect)
    };
    if workspace_bytes < ambient_home_workspace_bytes()? {
        return Err("Home projection workspace must be explicitly funded".into());
    }
    let kind = if resident {
        conduit_semantic_catalog::ambient_resident_contributor_kind(aspect)?
    } else {
        conduit_semantic_catalog::ambient_aspect_contributor_kind(aspect)?
    };
    let digest = semantic_digest(
        implementation,
        concat!(
            include_str!("ambient_home.rs"),
            include_str!("../../src/ambient_home_offer.rs"),
            include_str!("../../src/installed_std/ambient_home_back.rs"),
            include_str!("../../src/installed_std/ambient_home_factory.rs"),
            include_str!("../../src/installed_std/ambient_home_route.rs"),
            include_str!("../../src/installed_std/body_kernel/home_face_route.rs"),
            include_str!("../../../../plots/ambient-display/model/src/contributor.rs"),
            include_str!("../../../../plots/ambient-display/model/src/face.rs"),
            include_str!("../../../../plots/ambient-display/model/src/backing.rs"),
            include_str!("../../../../plots/ambient-display/model/src/projection_memory.rs"),
            include_str!("../../../../plots/ambient-display/model/src/home.rs"),
            include_str!("../../../../plots/ambient-display/model/src/aspect.rs"),
            include_str!("../../../../plots/ambient-display/model/src/resident.rs"),
            include_str!("../../../../plots/ambient-display/model/src/availability.rs"),
            include_str!("../../../../plots/ambient-display/model/src/civil.rs"),
            include_str!("../../../../plots/ambient-display/model/src/types.rs"),
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
            execution_profile_id: profile.into(),
            implementation_id: implementation.into(),
            artifact_id: artifact.into(),
            host_calls: [project, custody]
                .into_iter()
                .map(|contract| HostCallRequirement {
                    contract_id: contract.into(),
                    target_kind: Some(kind.kind_id.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: conduit_semantic_catalog::AMBIENT_HOME_MAXIMUM_BYTES,
                    maximum_output_bytes: 0,
                })
                .collect(),
            resource_requirements: vec![resource_requirement(
                RUNTIME_MEMORY_RESOURCE_CLASS,
                workspace_bytes,
            )],
            authority_requirements: vec![],
        },
    )
    .build())
}

pub fn ambient_home_workspace_bytes() -> Result<u32, String> {
    // This immutable code-derived scalar owns no persistent heap. Its first
    // calculation remains preparation work, never a Host Call side effect.
    static WORKSPACE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    if let Some(value) = WORKSPACE.get() {
        return Ok(*value);
    }
    let value = conduit_semantic_catalog::ambient_home_workspace_bytes()?;
    let _ = WORKSPACE.set(value);
    Ok(value)
}
pub fn ambient_home_cpu_offer() -> Result<CapabilityOffer, String> {
    ambient_home_std_offer(ambient_home_workspace_bytes()?)
}

pub fn ambient_aspect_cpu_offer(aspect: AmbientDisplayAspect) -> Result<CapabilityOffer, String> {
    ambient_aspect_std_offer(aspect, ambient_home_workspace_bytes()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separate_aspects_keep_distinct_exact_calls_and_semantics() {
        let wallpaper = ambient_aspect_cpu_offer(AmbientDisplayAspect::Wallpaper).unwrap();
        let clock = ambient_aspect_cpu_offer(AmbientDisplayAspect::ClockWeather).unwrap();
        assert_ne!(wallpaper.kind_id, clock.kind_id);
        assert_ne!(
            wallpaper.implementation.implementation_id,
            clock.implementation.implementation_id
        );
        assert_ne!(wallpaper.host_calls, clock.host_calls);
        assert_eq!(wallpaper.inputs, clock.inputs);
        for offer in [wallpaper, clock] {
            assert_eq!(offer.host_calls.len(), 2);
            assert!(offer.host_calls[0].contract_id < offer.host_calls[1].contract_id);
            assert!(offer
                .host_calls
                .iter()
                .all(|call| call.target_kind.as_ref() == Some(&offer.kind_id)));
            assert!(offer.outputs.is_empty());
        }
    }
    #[test]
    fn ambient_home_offer_requires_derived_workspace_and_exact_call_order() {
        let minimum = ambient_home_workspace_bytes().unwrap();
        assert!(minimum > 0);
        assert!(ambient_home_std_offer(minimum - 1).is_err());
        let offer = ambient_home_cpu_offer().unwrap();
        assert_eq!(offer.resource_requirements[0].units, minimum);
        assert_eq!(
            offer.host_calls[0].contract_id.as_str(),
            AMBIENT_HOME_PROJECT_CALL
        );
        assert_eq!(
            offer.host_calls[1].contract_id.as_str(),
            AMBIENT_HOME_CUSTODY_CALL
        );
        assert!(offer.host_calls[0].contract_id < offer.host_calls[1].contract_id);
    }
}
