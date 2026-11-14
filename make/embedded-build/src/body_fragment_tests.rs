//! The build projection must preserve exact content, not infer it from a pool.
use super::*;
use conduit_core as c;

#[test]
fn projected_content_preserves_contract_residence_and_plain_rows() {
    let mut fragment = crate::tests::sealed_current_fragment();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let mut generated =
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING).unwrap();
    let content = c::ResourceContentOffer {
        contract: c::ResourceContentRequirement {
            identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            content_profile: "recording/chunk@1".into(),
            maximum_bytes: 4096,
            maximum_items: 64,
            retention: c::ResourceRetention::ExternalDurable,
            sharing: c::ResourceSharing::SingleWriterPublished,
            access: c::ResourceAccessMode::WriteCandidatePublish,
            generation_slots: 2,
            reader_leases: 1,
            publication_slots: 1,
            sensitive: true,
        },
        owner_host: fragment.host_id.clone(),
        owner_boot: fragment.boot_id.clone(),
        base_id: "base/flash".into(),
        residence_profile: "residence/nor@1".into(),
    };
    for index in 0..2 {
        fragment.placements[index]
            .resources
            .push(c::ResourceBinding {
                pool_id: format!("pool/{index}").into(),
                class_id: "resource/storage@1".into(),
                units: 4096,
                protected: None,
                compute: None,
                content: (index == 0).then(|| content.clone()),
            });
        generated.resources.push(GeneratedStaticResource {
            node: index as u16,
            resource: 0,
            units: 4096,
        });
    }
    // This exercises projection correspondence only; the mutated fixture is not
    // represented as a checked/sealed Plan or as execution authorization.
    let bytes = projected_resources(&fragment, &generated).unwrap();
    let rows = conduit_body::FragmentResources::new(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows.len(), 2);
    let first = rows[0].content.unwrap();
    assert_eq!(
        first.contract,
        conduit_body::FragmentContentContract::from_core(&content.contract)
    );
    assert_eq!(first.base, content.base_id.as_str());
    assert_eq!(first.residence_profile, content.residence_profile.as_str());
    assert!(rows[1].content.is_none());
    assert!(conduit_body::PlainFragmentResources::new(&bytes)
        .next()
        .unwrap()
        .is_err());
    fragment.placements[0].resources[0]
        .content
        .as_mut()
        .unwrap()
        .owner_boot = "foreign".into();
    assert!(projected_resources(&fragment, &generated).is_none());
    fragment.placements[0].resources[0].content = None;
    let plain = projected_resources(&fragment, &generated).unwrap();
    assert_eq!(
        conduit_body::PlainFragmentResources::new(&plain)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        2
    );
    generated.resources[0].units += 1;
    assert!(projected_resources(&fragment, &generated).is_none());
}
