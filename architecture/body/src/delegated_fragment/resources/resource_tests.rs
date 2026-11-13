//! CPU fixtures exercise signed projection custody/Core admission, not ingress authentication.
use super::*;
use crate::{
    admit_delegated_plain_resources, admit_delegated_resources,
    delegated_plain_resource_preparation_bytes, delegated_resource_preparation_bytes,
    FragmentContent, FragmentContentContract, FragmentResource, FragmentResources,
    PlainFragmentResource,
};
use alloc::{vec, vec::Vec};
use sha2::{Digest, Sha256};
fn contract(write: bool) -> c::ResourceContentRequirement {
    c::ResourceContentRequirement {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        version: c::ResourceVersionIdentity::from_digest([2; 32]),
        content_profile: "text/json-utf8@1".into(),
        maximum_bytes: 4096,
        maximum_items: 1,
        retention: c::ResourceRetention::ExternalDurable,
        sharing: c::ResourceSharing::SingleWriterPublished,
        access: if write {
            c::ResourceAccessMode::WriteCandidatePublish
        } else {
            c::ResourceAccessMode::ReadPublished
        },
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: u16::from(write),
        sensitive: false,
    }
}
fn host(contract: &c::ResourceContentRequirement) -> c::HostAdvertisement {
    let mut pools = Vec::new();
    for name in ["pool/one", "pool/two"] {
        let mut pool = c::resource_offer(name, "resource/snapshot@1", 1);
        pool.content = Some(c::ResourceContentOffer {
            contract: contract.clone(),
            owner_host: "host".into(),
            owner_boot: "boot".into(),
            base_id: "flash/base".into(),
            residence_profile: "flash/snapshot@1".into(),
        });
        pools.push(pool);
    }
    c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host".into(),
        boot_id: "boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "profile".into(),
        bases: vec![],
        resources: pools,
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn observations(host: &c::HostAdvertisement) -> Vec<c::ResourceObservation> {
    host.resources
        .iter()
        .map(|pool| c::ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: c::ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id: pool.pool_id.as_str().into(),
        })
        .collect()
}
fn row<'a>(contract: &'a c::ResourceContentRequirement, second: bool) -> FragmentResource<'a> {
    FragmentResource {
        binding: PlainFragmentResource {
            node: u16::from(second),
            resource: 0,
            placement: if second {
                "placement/two"
            } else {
                "placement/one"
            },
            pool: if second { "pool/two" } else { "pool/one" },
            class: "resource/snapshot@1",
            units: 1,
        },
        content: Some(FragmentContent {
            contract: FragmentContentContract::from_core(contract),
            base: "flash/base",
            residence_profile: "flash/snapshot@1",
        }),
    }
}
fn projection(rows: &[FragmentResource<'_>]) -> Vec<u8> {
    let mut out = vec![0; c::ASSIGNED_PLAN_HEADER_BYTES];
    out[..8].copy_from_slice(b"CNDAP002");
    out[8..10].copy_from_slice(&c::ASSIGNED_PLAN_SCHEMA.to_le_bytes());
    for row in rows {
        out.push(c::ASSIGNED_RESOURCE);
        out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(&row.node.to_le_bytes());
        out.extend_from_slice(&row.resource.to_le_bytes());
        out.extend_from_slice(&row.units.to_le_bytes());
    }
    let length = out.len() as u16;
    out[10..12].copy_from_slice(&length.to_le_bytes());
    out
}
fn fixture<'a>(bindings: &'a [u8], projection: &'a [u8]) -> DelegatedBodyFragment<'a> {
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/local-assigned-fragments@1");
    hash.update(8u32.to_le_bytes());
    hash.update(b"fragment");
    let metadata = BodyFragmentApprovalMetadata {
        schema: 3,
        body: "body",
        wake: "wake",
        body_plan: "body-plan",
        body_play: "body-play",
        play_sequence: 1,
        workload_revision: 1,
        source: "source",
        checked_plot: "checked",
        expanded_plot: "expanded",
        plot_plan: "plot-plan",
        fragment: "fragment",
        host: "host",
        boot: "boot",
        offer_generation: 1,
        local_fragment_count: 1,
        local_fragment_digest: hash.finalize().into(),
        resource_bindings: bindings,
        fore_ports: &[],
        projection_length: projection.len() as u32,
        projection_digest: Sha256::digest(projection).into(),
    };
    let id = c::AssignedIdentity::from_text;
    let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
    counts[0] = 2;
    DelegatedBodyFragment {
        metadata,
        projection,
        view: c::AssignedPlanView {
            plan: id("plot-plan"),
            fragment: id("fragment"),
            host: id("host"),
            boot: id("boot"),
            encoded_bytes: projection.len() as u16,
            runtime_state_bytes: 0,
            counts,
        },
        receiver_profile: id("profile"),
    }
}
#[test]
fn exact_signed_write_and_read_contracts_reach_real_core_admission() {
    for write in [false, true] {
        let contract = contract(write);
        let row = row(&contract, false);
        let mut bytes = vec![];
        row.append_to(&mut bytes).unwrap();
        let projection = projection(&[row]);
        let fragment = fixture(&bytes, &projection);
        let host = host(&contract);
        let observations = observations(&host);
        let mut owner = c::ResourceAdmissionOwner::new(host);
        assert!(admit_delegated_plain_resources(&[&fragment], &mut owner, &observations).is_err());
        assert!(owner.admissions().is_empty());
        assert!(
            delegated_plain_resource_preparation_bytes(&[&fragment], &owner, &observations)
                .is_err()
        );
        let bound =
            delegated_resource_preparation_bytes(&[&fragment], &owner, &observations).unwrap();
        let admitted = admit_delegated_resources(&[&fragment], &mut owner, &observations).unwrap();
        assert_eq!(admitted.len(), 1);
        let item = &admitted[0].items[0];
        assert_eq!(item.requirement.content.as_ref(), Some(&contract));
        assert_eq!(item.binding.content.as_ref().unwrap().contract, contract);
        // Inline Option payloads are included in ResourceAdmissionItem; these are
        // the exact additional owned string bytes over the existing plain bound.
        let content_bytes = 2 * contract.content_profile.as_str().len()
            + "host".len()
            + "boot".len()
            + "flash/base".len()
            + "flash/snapshot@1".len();
        let mut plain_bytes = vec![];
        row.binding.append_to(&mut plain_bytes).unwrap();
        let plain_fragment = fixture(&plain_bytes, &projection);
        let mut plain_host = owner.host().clone();
        for pool in &mut plain_host.resources {
            pool.content = None;
        }
        let plain_owner = c::ResourceAdmissionOwner::new(plain_host);
        let plain_bound = delegated_plain_resource_preparation_bytes(
            &[&plain_fragment],
            &plain_owner,
            &observations,
        )
        .unwrap();
        assert_eq!(bound, plain_bound + content_bytes);
    }
}
#[test]
fn signed_contract_and_residence_mismatches_refuse_atomically() {
    let expected = contract(true);
    for change in 0..6 {
        let mut changed = expected.clone();
        if change == 0 {
            changed.version = c::ResourceVersionIdentity::from_digest([9; 32]);
        }
        if change == 1 {
            changed.maximum_bytes = 4095;
        }
        let mut row = row(&changed, false);
        if change == 2 {
            row.content.as_mut().unwrap().base = "foreign/base";
        }
        if change == 3 {
            row.content.as_mut().unwrap().residence_profile = "foreign/profile";
        }
        let mut bytes = vec![];
        row.append_to(&mut bytes).unwrap();
        let projection = projection(&[row]);
        let mut fragment = fixture(&bytes, &projection);
        let mut host = host(&expected);
        if change == 4 {
            host.resources[0].content.as_mut().unwrap().owner_boot = "foreign".into();
        }
        if change == 5 {
            fragment.metadata.boot = "foreign";
        }
        let observations = observations(&host);
        let mut owner = c::ResourceAdmissionOwner::new(host);
        assert!(admit_delegated_resources(&[&fragment], &mut owner, &observations).is_err());
        assert!(owner.admissions().is_empty());
    }
}
#[test]
fn late_batch_refusal_preserves_old_ledger_and_exact_projection_is_required() {
    let contract = contract(false);
    let first = row(&contract, false);
    let mut second = row(&contract, true);
    second.content.as_mut().unwrap().base = "foreign/base";
    let mut bytes = vec![];
    first.append_to(&mut bytes).unwrap();
    second.append_to(&mut bytes).unwrap();
    let projection = projection(&[first, second]);
    let fragment = fixture(&bytes, &projection);
    let host = host(&contract);
    let observations = observations(&host);
    let mut owner = c::ResourceAdmissionOwner::new(host);
    assert!(admit_delegated_resources(&[&fragment], &mut owner, &observations).is_err());
    assert!(owner.admissions().is_empty());
    // Establish unrelated retained custody first; a later failing atomic batch
    // must preserve it, including its exact contract and observation Signs.
    let old_row = row(&contract, true);
    let mut old_bytes = vec![];
    old_row.append_to(&mut old_bytes).unwrap();
    let old_projection = self::projection(&[old_row]);
    let old_fragment = fixture(&old_bytes, &old_projection);
    admit_delegated_resources(&[&old_fragment], &mut owner, &observations).unwrap();
    let previous = owner.admissions().to_vec();
    assert!(admit_delegated_resources(&[&fragment], &mut owner, &observations).is_err());
    assert_eq!(owner.admissions(), previous);
    let mut missing = vec![];
    first.append_to(&mut missing).unwrap();
    let fragment = fixture(&missing, &projection);
    assert!(delegated_resource_preparation_bytes(&[&fragment], &owner, &observations).is_err());
    assert!(admit_delegated_resources(&[&fragment], &mut owner, &observations).is_err());
}
#[test]
fn versioned_borrowed_codec_rejects_oversize_and_downgrade_or_mixed_streams() {
    let contract = contract(true);
    let row = row(&contract, false);
    let mut bytes = vec![];
    row.append_to(&mut bytes).unwrap();
    let decoded = FragmentResources::new(&bytes).next().unwrap().unwrap();
    let profile = decoded.content.unwrap().contract.content_profile;
    let range = bytes.as_ptr_range();
    assert!(range.contains(&profile.as_ptr()));
    assert!(crate::PlainFragmentResources::new(&bytes)
        .next()
        .unwrap()
        .is_err());
    let mut changed = bytes.clone();
    changed[6] = b'2';
    assert!(FragmentResources::new(&changed).next().unwrap().is_err());
    let mut legacy = vec![];
    row.binding.append_to(&mut legacy).unwrap();
    assert!(row.append_to(&mut legacy).is_err());
    let long = alloc::string::String::from_utf8(vec![b'x'; 129]).unwrap();
    let mut oversized = row;
    oversized.content.as_mut().unwrap().base = &long;
    assert!(oversized.append_to(&mut vec![]).is_err());
    let mut raw = bytes[..7].to_vec();
    raw.extend(postcard::to_allocvec(&oversized).unwrap());
    assert!(FragmentResources::new(&raw).next().unwrap().is_err());
}

#[test]
fn existing_owner_attribution_covers_exact_versioned_content_metadata_bytes() {
    use crate::{
        host_command_attribution_transcript, FederationPeer, HostCommandReceiveBounds,
        IssuedHostCommandChallenge, MembershipCredential,
    };
    use ed25519_dalek::{Signer, SigningKey};
    let contract = contract(true);
    let row = row(&contract, false);
    let mut bindings = vec![];
    row.append_to(&mut bindings).unwrap();
    let projection = projection(&[row]);
    let fragment = fixture(&bindings, &projection);
    let mut payload = postcard::to_allocvec(&fragment.metadata).unwrap();
    payload.extend_from_slice(&projection);
    let key = SigningKey::from_bytes(&[9; 32]);
    let member: MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id":"credential","body_id":"body","part_id":"part",
        "host_id":"host","boot_id":"boot","issued_at_millis":1,
    }))
    .unwrap();
    let issuer = FederationPeer {
        host_id: "issuer".into(),
        boot_id: "issuer-boot".into(),
        offer_generation: c::OfferGeneration(1),
        verifying_key: key.verifying_key().to_bytes(),
    };
    let host = host(&contract);
    let issue = || {
        IssuedHostCommandChallenge::issue(
            &member,
            &host,
            &issuer,
            HostCommandReceiveBounds {
                payload_contract: "fixture/body-fragment",
                maximum_payload_bytes: 4096,
                lifetime_millis: 10,
            },
            [1; 32],
            10,
        )
        .unwrap()
    };
    let slot = issue();
    let signature = key
        .sign(&host_command_attribution_transcript(slot.challenge(), &payload).unwrap())
        .to_bytes();
    assert!(slot.authenticate(&payload, &signature, 11).is_ok());
    // Change only signed residence facts, retaining the same numeric projection.
    let mut changed = row;
    changed.content.as_mut().unwrap().base = "other/base";
    let mut changed_bindings = vec![];
    changed.append_to(&mut changed_bindings).unwrap();
    let changed_fragment = fixture(&changed_bindings, &projection);
    let mut changed_payload = postcard::to_allocvec(&changed_fragment.metadata).unwrap();
    changed_payload.extend_from_slice(&projection);
    assert!(issue()
        .authenticate(&changed_payload, &signature, 11)
        .is_err());
}

#[test]
fn start_custody_comparison_requires_exact_content_and_residence() {
    let contract = contract(true);
    let row = row(&contract, false);
    let mut bytes = vec![];
    row.append_to(&mut bytes).unwrap();
    let projection = projection(&[row]);
    let fragment = fixture(&bytes, &projection);
    let host = host(&contract);
    let observations = observations(&host);
    let mut owner = c::ResourceAdmissionOwner::new(host);
    let admitted = admit_delegated_resources(&[&fragment], &mut owner, &observations).unwrap();
    let original = &admitted[0].items[0];
    assert!(row.matches_admitted_item(original, "host", "boot"));
    assert!(!row.matches_admitted_item(original, "host", "other-boot"));
    for change in 0..17 {
        let mut changed = original.clone();
        match change {
            0 => changed.binding.pool_id = "other-pool".into(),
            1 => changed.binding.units += 1,
            2 => changed.requirement.content = None,
            3 => changed.binding.content = None,
            4 => changed.requirement.content.as_mut().unwrap().maximum_bytes -= 1,
            5 => {
                changed.requirement.content.as_mut().unwrap().version =
                    c::ResourceVersionIdentity::from_digest([9; 32])
            }
            6 => changed.binding.content.as_mut().unwrap().owner_host = "other-host".into(),
            7 => changed.binding.content.as_mut().unwrap().owner_boot = "other-boot".into(),
            8 => changed.binding.content.as_mut().unwrap().base_id = "other-base".into(),
            9 => {
                changed.binding.content.as_mut().unwrap().residence_profile =
                    "other-residence".into()
            }
            10 => {
                changed.binding.content.as_mut().unwrap().contract.access =
                    c::ResourceAccessMode::ReadPublished
            }
            11 => {
                changed
                    .binding
                    .content
                    .as_mut()
                    .unwrap()
                    .contract
                    .generation_slots += 1
            }
            12 => {
                changed
                    .binding
                    .content
                    .as_mut()
                    .unwrap()
                    .contract
                    .reader_leases += 1
            }
            13 => {
                changed
                    .binding
                    .content
                    .as_mut()
                    .unwrap()
                    .contract
                    .publication_slots += 1
            }
            14 => changed.binding.content.as_mut().unwrap().contract.sensitive = true,
            15 => {
                changed
                    .binding
                    .content
                    .as_mut()
                    .unwrap()
                    .contract
                    .maximum_items += 1
            }
            16 => {
                changed
                    .binding
                    .content
                    .as_mut()
                    .unwrap()
                    .contract
                    .content_profile = "other-profile".into()
            }
            _ => unreachable!(),
        }
        assert!(
            !row.matches_admitted_item(&changed, "host", "boot"),
            "mutation {change}"
        );
    }
    let plain = FragmentResource {
        content: None,
        ..row
    };
    assert!(!plain.matches_admitted_item(original, "host", "boot"));
    let mut plain_item = original.clone();
    plain_item.requirement.content = None;
    plain_item.binding.content = None;
    assert!(plain.matches_admitted_item(&plain_item, "host", "boot"));
    assert!(!row.matches_admitted_item(&plain_item, "host", "boot"));
}
