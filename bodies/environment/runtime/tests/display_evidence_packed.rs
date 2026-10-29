//! Lossless fixed-profile evidence storage; no event/identity/quota reduction.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::display_evidence::{self as wire, packed, Page, Refusal};
fn raw(count: usize, remote: usize, flags: u8) -> Vec<u8> {
    let mut raw = vec![0; wire::HEADER_BYTES + count * wire::RECORD_BYTES];
    raw[..4].copy_from_slice(b"CEV1");
    raw[4..6].copy_from_slice(&1u16.to_le_bytes());
    raw[6..8].copy_from_slice(&(count as u16).to_le_bytes());
    raw[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    for (i, r) in raw[16..].chunks_exact_mut(32).enumerate() {
        r[..4].copy_from_slice(&(i as u32).to_le_bytes());
        r[4..6].copy_from_slice(&u16::MAX.to_le_bytes());
        if i < remote {
            r[6] = 1;
            r[7..9].copy_from_slice(&u16::MAX.to_le_bytes());
            r[14] = 6 + (i % 6) as u8;
            r[15] = 1;
            r[16..20].fill(255);
            r[20] = u8::from(r[14] < 10);
            r[21..29].copy_from_slice(&u64::MAX.to_le_bytes());
        } else {
            r[14] = 17;
            if flags & 1 != 0 {
                r[6] = 1;
                r[7..9].copy_from_slice(&u16::MAX.to_le_bytes());
            }
            if flags & 2 != 0 {
                r[9] = 1;
                r[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
            }
        }
    }
    raw
}
fn roundtrip(raw: &[u8]) -> Vec<u8> {
    let mut packed = [0; packed::MAXIMUM_BYTES];
    let n = packed::encode(raw, &mut packed).unwrap();
    let mut restored = [0; wire::MAXIMUM_BYTES];
    let m = packed::decode(&packed[..n], &mut restored).unwrap();
    assert_eq!(&restored[..m], raw);
    assert_eq!(Page::parse(&restored[..m]).unwrap().invocation(), u64::MAX);
    let mut again = [0; packed::MAXIMUM_BYTES];
    assert_eq!(packed::encode(&restored[..m], &mut again).unwrap(), n);
    assert_eq!(&again[..n], &packed[..n]);
    packed[..n].to_vec()
}
#[test]
fn complete_maximal_profile_full_width_ids_and_all_options_are_lossless_without_allocation() {
    let mut zero = raw(1, 1, 0);
    zero[8..16].fill(0);
    zero[20..22].fill(0); // node
    zero[23..25].fill(0); // port
    zero[32..36].fill(0); // endpoint/cord
    zero[37..45].fill(0); // remote sequence
    let mut compact = [0; packed::MAXIMUM_BYTES];
    let n = packed::encode(&zero, &mut compact).unwrap();
    let mut restored = [0; wire::MAXIMUM_BYTES];
    let m = packed::decode(&compact[..n], &mut restored).unwrap();
    assert_eq!(&restored[..m], zero.as_slice());
    assert_eq!(Page::parse(&restored[..m]).unwrap().invocation(), 0);
    for flags in 0..4 {
        roundtrip(&raw(1, 0, flags));
    }
    for tag in 0..=22 {
        let mut page = raw(1, usize::from((6..=11).contains(&tag)), 3);
        page[16 + 14] = tag;
        if (6..=11).contains(&tag) {
            page[16 + 20] = u8::from(tag < 10);
        }
        roundtrip(&page);
    }
    for remote in 0..=12 {
        roundtrip(&raw(64, remote, 3));
    }
    let page = raw(64, 12, 3);
    let mut packed = [0; packed::MAXIMUM_BYTES];
    let mut restored = [0; wire::MAXIMUM_BYTES];
    let (_, memory) = allocation_probe::observe(|| {
        for _ in 0..960 {
            let n = packed::encode(&page, &mut packed).unwrap();
            assert_eq!(n, 764);
            let m = packed::decode(&packed[..n], &mut restored).unwrap();
            assert_eq!(&restored[..m], page.as_slice());
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn zero_oversized_remote_overquota_and_noncanonical_raw_refuse() {
    let mut output = [0; packed::MAXIMUM_BYTES];
    assert_eq!(
        packed::encode(&raw(0, 0, 0), &mut output),
        Err(Refusal::Geometry)
    );
    assert_eq!(
        packed::encode(&raw(65, 0, 0), &mut output),
        Err(Refusal::Geometry)
    );
    assert_eq!(
        packed::encode(&raw(13, 13, 0), &mut output),
        Err(Refusal::Capacity)
    );
    for offset in [0, 4, 16, 16 + 6, 16 + 9, 16 + 14, 16 + 15, 16 + 29] {
        let mut page = raw(1, 0, 0);
        page[offset] = 255;
        assert!(
            packed::encode(&page, &mut output).is_err(),
            "offset={offset}"
        );
    }
    let mut page = raw(1, 0, 0);
    page[16 + 7] = 1;
    assert_eq!(packed::encode(&page, &mut output), Err(Refusal::Canonical));
    assert_eq!(
        packed::encode(&raw(1, 0, 0), &mut output[..763]),
        Err(Refusal::Capacity)
    );
}
#[test]
fn packed_header_flags_tags_directions_count_footer_and_every_truncation_refuse() {
    let encoded = roundtrip(&raw(12, 12, 0));
    let mut restored = [0; wire::MAXIMUM_BYTES];
    for n in 0..encoded.len() {
        assert!(
            packed::decode(&encoded[..n], &mut restored).is_err(),
            "truncated={n}"
        );
    }
    let mut extra = encoded.clone();
    extra.push(0);
    assert_eq!(
        packed::decode(&extra, &mut restored),
        Err(Refusal::Canonical)
    );
    for offset in [0, 4, 6, 16 + 2, 16 + 3, 16 + 10] {
        let mut corrupt = encoded.clone();
        corrupt[offset] = 255;
        assert!(
            packed::decode(&corrupt, &mut restored).is_err(),
            "offset={offset}"
        );
    }
    for flags in [4, 6, 7, 8, 255] {
        let mut corrupt = encoded.clone();
        corrupt[16 + 3] = flags;
        assert!(
            packed::decode(&corrupt, &mut restored).is_err(),
            "flags={flags}"
        );
    }
    let mut wrong_direction = encoded.clone();
    wrong_direction[16 + 10] = 0;
    assert_eq!(
        packed::decode(&wrong_direction, &mut restored),
        Err(Refusal::Identity)
    );
    let mut wrong_kind = encoded.clone();
    wrong_kind[16 + 2] = 17;
    assert_eq!(
        packed::decode(&wrong_kind, &mut restored),
        Err(Refusal::Identity)
    );
    let mut over_remote = encoded.clone();
    over_remote[6..8].copy_from_slice(&13u16.to_le_bytes());
    over_remote.extend_from_slice(&encoded[16..35]);
    assert_eq!(
        packed::decode(&over_remote, &mut restored),
        Err(Refusal::Capacity)
    );
    for count in [0u16, 65, u16::MAX] {
        let mut corrupt = encoded.clone();
        corrupt[6..8].copy_from_slice(&count.to_le_bytes());
        assert_eq!(
            packed::decode(&corrupt, &mut restored),
            Err(Refusal::Geometry)
        );
    }
    assert_eq!(
        packed::decode(&encoded, &mut restored[..2063]),
        Err(Refusal::Capacity)
    );
    let mut too_long = vec![0; 765];
    too_long[..encoded.len()].copy_from_slice(&encoded);
    assert_eq!(
        packed::decode(&too_long, &mut restored),
        Err(Refusal::Geometry)
    );
}
