//! Owner export generation gate with real admission and sealed-file verification.
//! This fixture deliberately lacks original IMAGE-funded retained Source custody:
//! reaching that refusal is not positive signed-export or execution evidence.
use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use crate::durable_host::{self as installed, source_continuation as output};
use conduit_body::{continuation_recipe as recipe, ContinuationBundleEntry};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn sealed_selection(
    root: &Path,
    member: &crate::durable_host::owner::MemberPresentationBasis,
) -> output::OutputSelection {
    let checked = source();
    let source_digest = Sha256::digest(SOURCE.as_bytes()).into();
    let placement_ids = ["fixture/placement"];
    let pin = ContinuationBundleEntry {
        name: &checked.expanded.name,
        source_document: checked.expanded.source_document_id.as_str(),
        checked_plot: checked.expanded.checked_plot_id.as_str(),
        expanded_plot: checked.expanded.expanded_plot_id.as_str(),
        projection_digest: [2; 32],
        recipe_digest: [3; 32],
        placement_ids: &placement_ids,
        placement_nodes: &[0],
    };
    // These are inert recipe bytes, never a fabricated PreparedOutput token.
    // export must reread and verify the real finite bundle and recipe digest.
    let document = recipe::Document {
        schema: recipe::SCHEMA.into(),
        image_id: "fixture/image".into(),
        configuration_id: member.host_advertisement.profile.as_str().into(),
        source_digest,
        workset_digest: conduit_body::continuation_bundle_digest(source_digest, &[pin]).unwrap(),
        entries: vec![recipe::Entry {
            name: checked.expanded.name.clone(),
            source_document: checked.expanded.source_document_id.as_str().into(),
            checked_plot: checked.expanded.checked_plot_id.as_str().into(),
            expanded_plot: checked.expanded.expanded_plot_id.as_str().into(),
            projection_digest: [2; 32],
            recipe_digest: [3; 32],
            placement_ids: vec![placement_ids[0].into()],
            placement_nodes: vec![0],
        }],
    };
    document.checked_digest().unwrap();
    let bytes = serde_json::to_vec(&document).unwrap();
    fs::write(root.join("continuation-recipe.json"), &bytes).unwrap();
    let file_digest = installed::digest(&bytes);
    let mut aggregate = Sha256::new();
    aggregate.update(b"conduit.release/host-bundle-content@1\0");
    aggregate.update(b"continuation-recipe.json\0");
    aggregate.update(file_digest.as_bytes());
    aggregate.update(b"\n");
    let bundle_digest = format!("sha256:{:x}", aggregate.finalize());
    let manifest = conduit_host_make::HostReleaseManifest {
        schema: conduit_host_make::HOST_RELEASE_BUNDLE_SCHEMA.into(),
        target_id: "fixture/target".into(),
        make_package_id: "fixture/package".into(),
        output: "fixture/output".into(),
        builder_adapter: "fixture/builder".into(),
        deployment_adapter: "fixture/deployment".into(),
        source_identity: "fixture/source".into(),
        bundle_sha256: bundle_digest.clone(),
        files: vec![conduit_host_make::HostReleaseFile {
            path: "continuation-recipe.json".into(),
            bytes: bytes.len() as u64,
            sha256: file_digest,
            media_type: "application/json".into(),
        }],
        reviewed_native_build: None,
        target_metadata: Default::default(),
    };
    installed::write_json_atomic(&root.join("manifest.json"), &manifest).unwrap();
    installed::write_json_atomic(&root.join("member.json"), member).unwrap();
    output::OutputSelection {
        host_id: member.credential.host_id.as_str().into(),
        member_basis: root.join("member.json"),
        make_manifest: root.join("manifest.json"),
        make_bundle_sha256: bundle_digest,
        make_source_identity: manifest.source_identity,
        target_id: manifest.target_id,
        package_id: manifest.make_package_id,
        output: manifest.output,
        builder_adapter: manifest.builder_adapter,
        deployment_adapter: manifest.deployment_adapter,
        artifact_output: root.join("unpublished-prefix.bin"),
        membership_generation: 1,
        intent_generation: 1,
        ceilings: conduit_body::ContinuationCeilings {
            assignment_bytes: 4096,
            runtime_bytes: 16384,
            durable_bytes: 8192,
            operation_slots: 32,
            active_worksets: 1,
        },
    }
}

#[test]
fn member_local_generation_one_passes_global_membership_revision_gate_without_granting_source() {
    let (mut owner, root) = owner_fixture();
    let (credential, _, ad) = admit_with_key(&mut owner, &root, "host/continuation-member");
    admit_with_key(&mut owner, &root, "host/continuation-sibling");
    assert!(owner.session.evidence().membership.revision > 1);
    let member = crate::durable_host::owner::MemberPresentationBasis::new(credential, ad).unwrap();
    let selection = sealed_selection(&root, &member);
    assert_eq!(selection.membership_generation, 1);
    let before = owner.session.evidence().clone();
    assert_eq!(
        output::export(&owner, std::slice::from_ref(&selection)).unwrap_err(),
        "checked selected Source custody absent"
    );
    // The old global/local equality rejected above before Source custody. Exact
    // accepted member/configuration checks must still precede that same boundary.
    let mut wrong_host = selection.clone();
    wrong_host.host_id = "host/other".into();
    assert_eq!(
        output::export(&owner, &[wrong_host]).unwrap_err(),
        "selected continuation differs from actual accepted member/configuration"
    );
    let mut wrong_config = member.clone();
    wrong_config.host_advertisement.profile = "fixture/other-configuration".into();
    installed::write_json_atomic(&selection.member_basis, &wrong_config).unwrap();
    assert_eq!(
        output::export(&owner, std::slice::from_ref(&selection)).unwrap_err(),
        "member offer differs from current admitted observation"
    );
    // Independently seal a mismatching recipe while restoring the actual current
    // member basis: observed-offer verification passes, exact recipe scope fails.
    let mismatched_recipe = sealed_selection(&root, &wrong_config);
    installed::write_json_atomic(&selection.member_basis, &member).unwrap();
    assert_eq!(
        output::export(&owner, &[mismatched_recipe]).unwrap_err(),
        "selected continuation differs from actual accepted member/configuration"
    );
    let selection = sealed_selection(&root, &member);
    owner.member_leave(&root, &member.credential).unwrap();
    assert_eq!(
        output::export(&owner, std::slice::from_ref(&selection)).unwrap_err(),
        "native-mask-part-unavailable"
    );
    assert_eq!(owner.session.evidence().body, before.body);
    assert!(!selection.artifact_output.exists());
    fs::remove_dir_all(root).unwrap();
}
