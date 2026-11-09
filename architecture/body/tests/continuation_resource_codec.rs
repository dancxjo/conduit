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
fn typed_content_still_refuses_incomplete_continuation_recipe() {
    let mut plan = support::plan("recording", "host");
    let fragment = &mut plan.fragments[0];
    assert!(c::FragmentSealRecipe::required_limits(fragment).is_ok());
    fragment.placements[0].resources[0].content = Some(c::ResourceContentOffer {
        contract: contract(),
        owner_host: "host".into(),
        owner_boot: fragment.boot_id.clone(),
        base_id: "flash/base".into(),
        residence_profile: "flash/recording@1".into(),
    });
    assert!(matches!(
        c::FragmentSealRecipe::required_limits(fragment),
        Err(c::FragmentSealRefusal::Unsupported)
    ));
    assert!(matches!(
        c::FragmentSealRecipe::capture(
            fragment,
            c::FragmentSealLimits {
                literal_bytes: 65536,
                pieces: 1024
            }
        ),
        Err(c::FragmentSealRefusal::Unsupported)
    ));
}
