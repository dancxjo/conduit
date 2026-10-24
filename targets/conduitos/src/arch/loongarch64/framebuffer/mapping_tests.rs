use super::*;
const VA: u64 = 0xffff_8000_8000_0000;
const PA: u64 = 0x8000_0000;
fn regime() -> Regime {
    Regime {
        crmd: 0xb0,
        pwcl: PWCL,
        pwch: PWCH,
        stlbps: 12,
        pgdh: 0x1000,
        direct_windows: [0x11, 0, 0, 0],
        physical_bits: 48,
        virtual_bits: 48,
        reduced_address_config: 0,
    }
}
fn leaf(pa: u64, mat: u64) -> u64 {
    pa | 1 | 2 | (mat << 4) | 256
}
#[test]
fn complete_four_level_walk_reads_exact_descriptor_addresses() {
    let mut seen = Vec::new();
    let evidence = validate(VA, PA, 4097, regime(), |address| {
        seen.push(address);
        Ok(match seen.len() {
            1 | 5 => 0x2000,
            2 | 6 => 0x3000,
            3 | 7 => 0x4000,
            4 => leaf(PA, 1),
            _ => leaf(PA + PAGE, 1),
        })
    })
    .unwrap();
    assert_eq!(
        seen,
        [
            0x1800, 0x2010, 0x3000, 0x4000, 0x1800, 0x2010, 0x3000, 0x4008
        ]
    );
    assert_eq!(evidence.checked_pages, 2);
    assert_eq!(evidence.descriptors_read, 8);
    assert_eq!(evidence.memory_access_type, 1);
}
#[test]
fn huge_hglobal_is_not_a_physical_address_bit() {
    let mut reads = 0;
    let evidence = validate(VA, PA, 4097, regime(), |_| {
        reads += 1;
        Ok(if reads % 2 == 1 {
            0x2000
        } else {
            leaf(PA, 2) | (1 << 6) | (1 << 12)
        })
    })
    .unwrap();
    assert_eq!(evidence.checked_pages, 2);
    assert_eq!(evidence.descriptors_read, 4);
    assert_eq!(evidence.memory_access_type, 2);
}
#[test]
fn permissions_attributes_alignment_and_physical_identity_are_independent() {
    for (value, expected) in [
        (leaf(PA, 1) & !2, Refusal::Permission),
        (leaf(PA, 1) & !256, Refusal::Permission),
        (leaf(PA, 1) | 12, Refusal::Permission),
        (leaf(PA, 3), Refusal::MemoryAttribute),
        (leaf(PA, 1) | (1 << 55), Refusal::Table),
        (leaf(PA + 2 * PAGE, 1) | (1 << 6), Refusal::Table),
        (leaf(0xc000_0000, 1) | (1 << 6), Refusal::PhysicalMismatch),
    ] {
        let mut reads = 0;
        assert_eq!(
            validate(VA, PA, PAGE, regime(), |_| {
                reads += 1;
                Ok(if reads == 1 { 0x2000 } else { value })
            }),
            Err(expected)
        );
    }
}
#[test]
fn unsupported_regimes_and_matching_direct_windows_never_read_tables() {
    for field in 0..8 {
        let mut r = regime();
        match field {
            0 => r.crmd |= 8,
            1 => r.pwcl = 0,
            2 => r.pwch = 0,
            3 => r.stlbps = 14,
            4 => r.pgdh = 0,
            5 => r.physical_bits = 64,
            6 => r.virtual_bits = 47,
            _ => r.reduced_address_config = 1,
        };
        assert_eq!(
            validate(VA, PA, PAGE, r, |_| panic!("unsupported regime")),
            Err(Refusal::Regime)
        );
    }
    let mut r = regime();
    r.direct_windows[1] = 0xf000_0000_0000_0011;
    assert_eq!(
        validate(VA, PA, PAGE, r, |_| panic!("DMW shadows page walk")),
        Err(Refusal::DirectWindow)
    );
}
#[test]
fn extents_and_unreadable_tables_refuse_without_fabricating_descriptors() {
    for (va, bytes) in [
        (VA + 1, PAGE),
        (VA, 0),
        (VA, MAX_BYTES + 1),
        (0x9000_0000_8000_0000, PAGE),
    ] {
        assert_eq!(
            validate(va, PA, bytes, regime(), |_| panic!("invalid extent")),
            Err(Refusal::Extent)
        );
    }
    assert_eq!(
        validate(VA, PA, PAGE, regime(), |_| Err(Refusal::Table)),
        Err(Refusal::Table)
    );
}

#[test]
fn one_scanout_cannot_mix_memory_access_types() {
    let mut reads = 0;
    assert_eq!(
        validate(VA, PA, 4097, regime(), |_| {
            reads += 1;
            Ok(match reads {
                1 | 5 => 0x2000,
                2 | 6 => 0x3000,
                3 | 7 => 0x4000,
                4 => leaf(PA, 1),
                _ => leaf(PA + PAGE, 0),
            })
        }),
        Err(Refusal::MemoryAttribute)
    );
}
