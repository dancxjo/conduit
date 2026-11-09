#![cfg(feature = "authenticated-admission")]
//! Content encoding remains distinct from unsupported continuation authority.
use conduit_body::*;
use conduit_core as c;
#[path = "prepared_fragment/support.rs"]
mod support;
fn contract() -> c::ResourceContentRequirement {
    c::ResourceContentRequirement {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        version: c::ResourceVersionIdentity::from_digest([2; 32]),
        content_profile: "recording/canonical@1".into(),
        maximum_bytes: 4096,
        maximum_items: 64,
        retention: c::ResourceRetention::ExternalDurable,
        sharing: c::ResourceSharing::SingleWriterPublished,
        access: c::ResourceAccessMode::WriteCandidatePublish,
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: 1,
        sensitive: false,
    }
}
#[test]
fn typed_rows_keep_exact_canonical_bytes_and_atomic_encoding_refusal() {
    let contract = contract();
    let row = FragmentResource {
        binding: PlainFragmentResource {
            node: 0,
            resource: 0,
            placement: "placement",
            pool: "pool",
            class: "resource/recording",
            units: 1,
        },
        content: Some(FragmentContent {
            contract: FragmentContentContract::from_core(&contract),
            base: "flash/base",
            residence_profile: "flash/recording@1",
        }),
    };
    let payload = postcard::to_allocvec(&row).unwrap();
    assert_eq!(row.encoded_length().unwrap(), payload.len());
    let mut bytes = Vec::new();
    row.append_to(&mut bytes).unwrap();
    assert_eq!(&bytes[7..], payload);
    assert_eq!(&bytes[..7], b"\xff\xff\xffRSC1");
    row.append_to(&mut bytes).unwrap();
    assert_eq!(&bytes[7 + payload.len()..], payload);
    let decoded: Vec<_> = FragmentResources::new(&bytes).map(Result::unwrap).collect();
    assert_eq!(decoded.len(), 2);
    assert_eq!(
        decoded[0].content.unwrap().contract,
        row.content.unwrap().contract
    );
    assert_eq!(decoded[1].content.unwrap().base, "flash/base");
    let previous = bytes.clone();
    let mut invalid = row;
    invalid.content.as_mut().unwrap().base = "";
    assert!(invalid.encoded_length().is_err());
    assert!(invalid.append_to(&mut bytes).is_err());
    assert_eq!(bytes, previous);
    let mut plain = Vec::new();
    row.binding.append_to(&mut plain).unwrap();
    let original = plain.clone();
    assert!(row.append_to(&mut plain).is_err());
    assert_eq!(plain, original, "no mixed-stream downgrade");
}
#[test]
fn typed_content_recipe_matches_full_core_seal_across_boots_and_refuses_drift() {
    use c::{FragmentScopeRole as R, FragmentSealValue as V};
    let mut plan = support::plan("recording", "host");
    let fragment = &mut plan.fragments[0];
    fragment.placements[0].resources[0].content = Some(c::ResourceContentOffer {
        contract: contract(),
        owner_host: "host".into(),
        owner_boot: fragment.boot_id.clone(),
        base_id: "flash/base".into(),
        residence_profile: "flash/recording@1".into(),
    });
    let recipe = c::FragmentSealRecipe::capture(
        fragment,
        c::FragmentSealRecipe::required_limits(fragment).unwrap(),
    )
    .unwrap();
    let pieces: Vec<_> = recipe.pieces().collect();
    for boot in ["boot/one", "boot/two"] {
        fragment.boot_id = boot.into();
        let p = &mut fragment.placements[0];
        p.boot_id = boot.into();
        p.resources[0].content.as_mut().unwrap().owner_boot = boot.into();
        let mut bindings = Vec::new();
        for piece in &pieces {
            let c::FragmentSealPiece::Field(field, _) = piece else {
                continue;
            };
            let value = if let Some(ordinal) = field.placement {
                let p = &fragment.placements[usize::from(ordinal)];
                match field.role {
                    R::Host => V::Text(p.host_id.as_str()),
                    R::Boot => V::Text(p.boot_id.as_str()),
                    R::OfferGeneration => V::U64(p.offer_generation.0),
                    R::PlacementId => V::Text(p.placement_id.as_str()),
                    R::ResourcePool => {
                        V::Text(p.resources[usize::from(field.entry)].pool_id.as_str())
                    }
                    R::ContentOwnerHost => V::Text(
                        p.resources[usize::from(field.entry)]
                            .content
                            .as_ref()
                            .unwrap()
                            .owner_host
                            .as_str(),
                    ),
                    R::ContentOwnerBoot => V::Text(
                        p.resources[usize::from(field.entry)]
                            .content
                            .as_ref()
                            .unwrap()
                            .owner_boot
                            .as_str(),
                    ),
                    _ => panic!("fixture has no other dynamic slots"),
                }
            } else {
                match field.role {
                    R::Host => V::Text(fragment.host_id.as_str()),
                    R::Boot => V::Text(fragment.boot_id.as_str()),
                    R::OfferGeneration => V::U64(fragment.offer_generation.0),
                    _ => panic!("header"),
                }
            };
            bindings.push(c::FragmentSealBinding {
                field: *field,
                value,
            });
        }
        let digest = c::fragment_seal_recipe_digest(&pieces, &bindings).unwrap();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, c::compute_fragment_id(fragment).as_str());
        let resource = &fragment.placements[0].resources[0];
        let offer = c::ResourceOffer {
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            capacity_units: resource.units,
            compute: None,
            content: resource.content.clone(),
        };
        assert!(c::fragment_seal_resource_matches(&pieces, 0, 0, &offer, resource.units).is_ok());
        for mutation in 0..5 {
            let mut changed = offer.clone();
            match mutation {
                0 => changed.content.as_mut().unwrap().contract.maximum_items += 1,
                1 => changed.content.as_mut().unwrap().base_id = "other/base".into(),
                2 => changed.content.as_mut().unwrap().residence_profile = "other/residence".into(),
                3 => changed.class_id = "other/class".into(),
                _ => changed.content = None,
            }
            assert!(
                c::fragment_seal_resource_matches(&pieces, 0, 0, &changed, resource.units).is_err()
            );
        }
    }
}
