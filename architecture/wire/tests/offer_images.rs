#![cfg(feature = "offer-images")]
use conduit_core as c;
use conduit_wire::{
    resolve_capability_offer_images as resolve, CapabilityOfferImage as Image,
    CapabilityOfferImageLimits as Limits, CapabilityOfferImageRefusal as R, DeflatedDocument,
};
use sha2::{Digest, Sha256};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

fn offer(id: &str) -> c::CapabilityOffer {
    c::capability_offer_from_parts! {
        semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
        capability_id: id.into(), kind_id: "test/operation".into(),
        kind_contract_revision: "test/operation@1".into(), inputs: vec![], outputs: vec![],
        implementation: c::ImplementationOffer {
            execution_profile_id: "test/profile".into(), implementation_id: "test/back".into(),
            artifact_id: "test/artifact".into(),
        },
        host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
        limits: c::CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 1 },
    }
}
fn encode(json: &[u8]) -> Vec<u8> {
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(json, 6);
    let document = DeflatedDocument::from_parts(&compressed, json.len()).unwrap();
    let mut image = vec![0; 8];
    document.write_header(&mut image).unwrap();
    image.extend(compressed);
    image
}
fn image(bytes: &[u8]) -> Image<'_> {
    Image {
        sha256: Sha256::digest(bytes).into(),
        bytes,
    }
}
fn limits() -> Limits {
    Limits {
        maximum_offers: 2,
        maximum_image_bytes: 4096,
        maximum_expanded_offer_bytes: 4096,
        maximum_total_image_bytes: 8192,
        maximum_total_expanded_bytes: 8192,
    }
}

#[test]
fn complete_typed_records_preserve_order_and_every_contract_field() {
    let expected = [offer("offer/second"), offer("offer/first")];
    let a = encode(&serde_json::to_vec(&expected[0]).unwrap());
    let b = encode(&serde_json::to_vec(&expected[1]).unwrap());
    let mut scratch = [0xa5; 4100];
    let actual = resolve(&[image(&a), image(&b)], &mut scratch[..4096], limits()).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(&scratch[4096..], &[0xa5; 4]);
    assert_eq!(resolve(&[], &mut [], limits()).unwrap(), []);
}

#[test]
fn local_count_per_image_aggregate_and_scratch_budgets_refuse_before_allocation_or_writes() {
    let bytes = encode(&serde_json::to_vec(&offer("offer/one")).unwrap());
    let expanded = DeflatedDocument::parse_image(&bytes)
        .unwrap()
        .expanded_bytes();
    let cases = [
        Limits {
            maximum_offers: 1,
            ..limits()
        },
        Limits {
            maximum_image_bytes: bytes.len() - 1,
            ..limits()
        },
        Limits {
            maximum_expanded_offer_bytes: expanded - 1,
            ..limits()
        },
        Limits {
            maximum_total_image_bytes: bytes.len() * 2 - 1,
            ..limits()
        },
        Limits {
            maximum_total_expanded_bytes: expanded * 2 - 1,
            ..limits()
        },
    ];
    for limit in cases {
        let mut scratch = [0xa5; 4096];
        let inputs = [image(&bytes), image(&bytes)];
        let (result, cost) = probe::observe(|| resolve(&inputs, &mut scratch, limit));
        assert_eq!(result, Err(R::Bounds));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert!(scratch.iter().all(|b| *b == 0xa5));
    }
    assert_eq!(
        resolve(&[image(&bytes)], &mut vec![0; expanded - 1], limits()),
        Err(R::Bounds)
    );
}

#[test]
fn every_digest_includes_header_and_is_checked_before_any_decoding() {
    let bytes = encode(&serde_json::to_vec(&offer("offer/one")).unwrap());
    for offset in [4, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        let inputs = [
            image(&bytes),
            Image {
                sha256: image(&bytes).sha256,
                bytes: &changed,
            },
        ];
        let mut scratch = [0xa5; 4096];
        let (result, cost) = probe::observe(|| resolve(&inputs, &mut scratch, limits()));
        // The modified extent still fits the local bound, but its reference is stale.
        assert_eq!(result, Err(R::Digest));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert!(scratch.iter().all(|b| *b == 0xa5));
    }
}

#[test]
fn malformed_encoding_contract_and_duplicate_offer_refuse_the_complete_batch() {
    let valid = encode(&serde_json::to_vec(&offer("offer/one")).unwrap());
    assert_eq!(
        resolve(&[image(&valid), image(&valid)], &mut [0; 4096], limits()),
        Err(R::DuplicateCapability)
    );
    let invalid_json = encode(b"not JSON");
    assert_eq!(
        resolve(
            &[image(&valid), image(&invalid_json)],
            &mut [0; 4096],
            limits()
        ),
        Err(R::Encoding)
    );
    let mut invalid = offer("offer/one");
    invalid.kind_id = "".into();
    let invalid_contract = encode(&serde_json::to_vec(&invalid).unwrap());
    assert_eq!(
        resolve(&[image(&invalid_contract)], &mut [0; 4096], limits()),
        Err(R::Contract)
    );
    let mut corrupt = valid.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        resolve(&[image(&corrupt)], &mut [0; 4096], limits()),
        Err(R::Document)
    );
    let mut suffix = valid.clone();
    suffix.push(0);
    assert_eq!(
        resolve(&[image(&suffix)], &mut [0; 4096], limits()),
        Err(R::Document)
    );
    assert_eq!(
        resolve(&[image(b"invalid")], &mut [0; 4096], limits()),
        Err(R::Document)
    );
}
