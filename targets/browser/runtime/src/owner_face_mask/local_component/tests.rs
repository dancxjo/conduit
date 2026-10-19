//! Source identity admission; DOM existence and accessibility are separate Host proof.
use super::*;
#[test]
fn descriptor_pins_exact_five_source_resources_and_bounded_graph() {
    let value = serde_json::to_value(descriptor()).unwrap();
    assert_eq!(value["resources"].as_array().unwrap().len(), 5);
    assert!(
        SOURCES
            .iter()
            .map(|(_, _, bytes)| bytes.len())
            .sum::<usize>()
            <= MAXIMUM_BUNDLE_BYTES
    );
    assert!(source_graph_within_bounds());
    assert_eq!(value["bundle_sha256"], expected_admission().bundle_sha256);
    for (resource, (role, path, bytes)) in
        value["resources"].as_array().unwrap().iter().zip(SOURCES)
    {
        assert_eq!(resource["role"], role);
        assert_eq!(resource["path"], path);
        assert_eq!(resource["bytes"], bytes.len());
        assert_eq!(resource["sha256"], format!("{:x}", Sha256::digest(bytes)));
    }
    println!(
        "portable Back bundle={} bytes={} descriptor={}",
        bundle_digest(),
        SOURCES
            .iter()
            .map(|(_, _, bytes)| bytes.len())
            .sum::<usize>(),
        value
    );
}
#[test]
fn unknown_missing_stale_or_mismatched_component_admission_never_selects_back() {
    let exact = expected_admission();
    assert!(RendererProfile::admit(true, Some(&exact)).is_ok());
    assert!(RendererProfile::admit(false, None).is_ok());
    assert!(RendererProfile::admit(true, None).is_err());
    assert!(RendererProfile::admit(false, Some(&exact)).is_err());
    let mut variants = vec![exact.clone(); 4];
    variants[0].identity.push_str("/forged");
    variants[1].version = 2;
    variants[2].tag.push_str("-forged");
    variants[3].bundle_sha256 = "0".repeat(64);
    for variant in variants {
        assert!(RendererProfile::admit(true, Some(&variant)).is_err());
    }
}
#[test]
fn pinned_profile_changes_local_back_identity_but_remote_profile_is_unchanged() {
    let host = conduit_core::HostId::from("host/component-pin");
    let boot = conduit_core::BootId::from("boot/component-pin");
    let original = crate::installed_browser::membership_advertisement(host.clone(), boot.clone());
    let ordinary = RendererProfile::Ordinary.host(host.clone(), boot.clone());
    assert_eq!(ordinary, original);
    let selected = RendererProfile::PortableComponent.host(host, boot);
    let configured = crate::installed_browser::dom_mask::configured_offer();
    assert!(ordinary.capabilities.contains(&configured));
    assert!(!selected.capabilities.contains(&configured));
    let pinned = selected
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_face::COMPONENT_MASK_KIND)
        .unwrap();
    assert_eq!(
        pinned.implementation.implementation_id.as_str(),
        IMPLEMENTATION
    );
    assert_eq!(
        pinned.implementation.artifact_id.as_str(),
        artifact(&bundle_digest())
    );
    assert_eq!(pinned.host_calls, configured.host_calls);
    assert_eq!(
        pinned.resource_requirements,
        configured.resource_requirements
    );
}
