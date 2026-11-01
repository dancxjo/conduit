use super::*;

#[test]
fn descriptor_pins_the_complete_finite_owner_component_graph() {
    let descriptor = owner_component_descriptor().unwrap();
    assert_eq!(descriptor.resources.len(), 6);
    assert_eq!(descriptor.maximum_bundle_bytes, 131_072);
    assert_eq!(
        descriptor
            .resources
            .iter()
            .map(|resource| resource.role)
            .collect::<Vec<_>>(),
        [
            "component",
            "presentation",
            "pointer",
            "style",
            "icons",
            "look"
        ]
    );
    for (resource, (role, path, maximum, bytes)) in descriptor.resources.iter().zip(SOURCES) {
        assert_eq!(resource.role, role);
        assert_eq!(resource.path, path);
        assert_eq!(resource.bytes, bytes.len());
        assert_eq!(resource.maximum_bytes, maximum);
        assert_eq!(resource.sha256, format!("{:x}", Sha256::digest(bytes)));
        assert!(resource.bytes <= resource.maximum_bytes);
    }
    let mut framed = b"conduit.browser/owner-face-bundle@1\n".to_vec();
    for (role, _, _, bytes) in SOURCES {
        framed.extend_from_slice(&(role.len() as u32).to_be_bytes());
        framed.extend_from_slice(role.as_bytes());
        framed.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        framed.extend_from_slice(bytes);
    }
    assert_eq!(
        descriptor.bundle_sha256,
        format!("{:x}", Sha256::digest(framed))
    );
}

#[test]
fn stale_or_foreign_admission_never_selects_the_owner_component() {
    let descriptor = owner_component_descriptor().unwrap();
    let original = OwnerComponentAdmission {
        identity: descriptor.identity.into(),
        version: descriptor.version,
        tag: descriptor.tag.into(),
        bundle_sha256: descriptor.bundle_sha256,
    };
    assert!(admits_owner_component(&original));
    let mut foreign = original.clone();
    foreign.identity.push_str("/foreign");
    assert!(!admits_owner_component(&foreign));
    let mut foreign = original.clone();
    foreign.version += 1;
    assert!(!admits_owner_component(&foreign));
    let mut foreign = original.clone();
    foreign.tag.push_str("-foreign");
    assert!(!admits_owner_component(&foreign));
    let mut foreign = original;
    foreign.bundle_sha256 = "0".repeat(64);
    assert!(!admits_owner_component(&foreign));
}

#[test]
fn pinned_offer_changes_only_realization_identity_on_existing_component_fore() {
    let generic = crate::offer();
    let configured = crate::configured_offer();
    let pinned = owner_component_offer().unwrap();
    let descriptor = owner_component_descriptor().unwrap();
    assert_eq!(
        pinned.implementation.artifact_id.as_str(),
        descriptor.artifact_id
    );
    assert_eq!(
        pinned.implementation.implementation_id.as_str(),
        descriptor.implementation_id
    );
    assert_eq!(pinned.capability_id.as_str(), descriptor.capability_id);
    assert_ne!(
        generic.implementation.artifact_id,
        pinned.implementation.artifact_id
    );
    let mut expected = configured;
    expected.capability_id = pinned.capability_id.clone();
    expected.implementation.implementation_id = pinned.implementation.implementation_id.clone();
    expected.implementation.artifact_id = pinned.implementation.artifact_id.clone();
    assert_eq!(pinned, expected);
}

#[test]
fn reviewed_static_graph_has_no_unpinned_dependency() {
    let source = |index: usize| core::str::from_utf8(SOURCES[index].3).unwrap();
    let dependencies = |text: &str| {
        text.lines()
            .filter(|line| {
                line.trim_start().starts_with("import ")
                    || (line.trim_start().starts_with("export ") && line.contains(" from "))
            })
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        dependencies(source(0)),
        [
            r#"import { OwnerFacePresentation, presentedBodySubject } from "./owner-face-presentation.mjs";"#,
            r#"export { ownerFacePointerReachable } from "./owner-face-pointer.mjs";"#,
        ]
    );
    assert_eq!(
        dependencies(source(1)),
        [
            r#"import { decodeVisualLook } from "../host/assets/application-look.mjs";"#,
            r#"import { decorate, iconForRole, iconForIntent } from "./owner-face-icons.mjs";"#,
            r#"import { ownerFacePointerReachable } from "./owner-face-pointer.mjs";"#,
        ]
    );
    for index in [2, 4, 5] {
        assert!(dependencies(source(index)).is_empty());
    }
    assert_eq!(source(0).matches("new URL(").count(), 1);
    assert!(source(0).contains("new URL('./owner-face-look.css', import.meta.url)"));
    for index in [0, 1, 2, 4, 5] {
        assert!(!source(index).contains("import("));
        assert!(!source(index).contains("import ("));
        if index != 0 {
            assert!(!source(index).contains("new URL("));
        }
    }
    let style = source(3).to_ascii_lowercase();
    assert!(!style.contains("@import"));
    assert!(!style.contains("url("));
}
