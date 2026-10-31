use conduit_environment_runtime::display_evidence_compact::{self as codec, Refusal};
#[test]
fn sparse_full_and_variable_extent_pages_are_lossless() {
    let basis = [0u8; 1168];
    let mut page = basis;
    let mut encoded = [0; 2070];
    let mut decoded = [0; 2064];
    for invocation in 0u64..19200 {
        page[8..16].copy_from_slice(&invocation.to_le_bytes());
        page[282..286].copy_from_slice(&(invocation as u32).to_le_bytes());
        let size = codec::encode(&basis, &page, &mut encoded).unwrap();
        assert!(size <= 38);
        let size = codec::decode(&basis, &encoded[..size], &mut decoded).unwrap();
        assert_eq!(&decoded[..size], &page);
    }
    for extent in [1, 17, 1168, 2064] {
        let page = vec![255; extent];
        let size = codec::encode(&basis, &page, &mut encoded).unwrap();
        assert_eq!(size, extent + 2);
        assert_eq!(
            codec::decode(&basis, &encoded[..size], &mut decoded).unwrap(),
            extent
        );
        assert_eq!(&decoded[..extent], &page);
    }
}
#[test]
fn malformed_or_short_capacity_never_changes_destination() {
    let basis = [0; 64];
    let mut page = basis;
    page[2] = 1;
    let mut encoded = [0; 66];
    let size = codec::encode(&basis, &page, &mut encoded).unwrap();
    let mut short = [42; 4];
    assert_eq!(
        codec::encode(&basis, &page, &mut short),
        Err(Refusal::Capacity)
    );
    assert_eq!(short, [42; 4]);
    let mut decoded = [42; 64];
    let mut invalid = encoded[..size].to_vec();
    invalid.extend_from_slice(&encoded[2..size]); // duplicate offset
    assert_eq!(
        codec::decode(&basis, &invalid, &mut decoded),
        Err(Refusal::Canonical)
    );
    assert_eq!(decoded, [42; 64]);
    invalid = encoded[..size].to_vec();
    invalid[2..4].copy_from_slice(&64u16.to_le_bytes());
    assert_eq!(
        codec::decode(&basis, &invalid, &mut decoded),
        Err(Refusal::Canonical)
    );
    assert_eq!(decoded, [42; 64]);
    invalid = encoded[..size].to_vec();
    invalid[4] = 0; // redundant replacement
    assert_eq!(
        codec::decode(&basis, &invalid, &mut decoded),
        Err(Refusal::Canonical)
    );
    assert_eq!(decoded, [42; 64]);
    for end in [0, 1, 3, 4] {
        assert!(codec::decode(&basis, &encoded[..end], &mut decoded).is_err());
        assert_eq!(decoded, [42; 64]);
    }
    assert!(codec::decode(&basis[..63], &encoded[..size], &mut decoded).is_err());
    assert_eq!(decoded, [42; 64]);
}
