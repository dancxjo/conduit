use super::{
    dtb::Dtb,
    handoff,
    memory::{Refusal, Region, admit},
};

struct Fixture {
    tree: Vec<u8>,
    strings: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            tree: vec![],
            strings: vec![],
        }
    }
    fn word(&mut self, value: u32) {
        self.tree.extend(value.to_be_bytes());
    }
    fn pad(&mut self) {
        while !self.tree.len().is_multiple_of(4) {
            self.tree.push(0);
        }
    }
    fn begin(&mut self, name: &[u8]) {
        self.word(1);
        self.tree.extend(name);
        self.tree.push(0);
        self.pad();
    }
    fn end(&mut self) {
        self.word(2);
    }
    fn prop(&mut self, name: &[u8], value: &[u8]) {
        let offset = self.strings.len();
        self.strings.extend(name);
        self.strings.push(0);
        self.word(3);
        self.word(value.len() as u32);
        self.word(offset as u32);
        self.tree.extend(value);
        self.pad();
    }
    fn cells(&mut self) {
        for name in [b"#address-cells".as_slice(), b"#size-cells"] {
            self.prop(name, &2u32.to_be_bytes());
        }
    }
    fn finish(mut self) -> Vec<u8> {
        self.word(9);
        let strings_offset = 56 + self.tree.len();
        let mut bytes = vec![0; 56];
        for (offset, value) in [
            (0, 0xd00dfeed),
            (4, (strings_offset + self.strings.len()) as u32),
            (8, 56),
            (12, strings_offset as u32),
            (16, 40),
            (20, 17),
            (24, 16),
            (32, self.strings.len() as u32),
            (36, self.tree.len() as u32),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        bytes.extend(self.tree);
        bytes.extend(self.strings);
        bytes
    }
}
fn reg(start: u64, bytes: u64) -> Vec<u8> {
    [start.to_be_bytes(), bytes.to_be_bytes()].concat()
}
fn fixture(board: &[u8], dynamic: bool, cells: bool, duplicate: bool) -> Vec<u8> {
    let mut f = Fixture::new();
    f.begin(b"");
    f.cells();
    f.prop(b"compatible", board);
    f.begin(b"memory@80000000");
    f.prop(b"reg", &reg(0x80000000, 0x100000000));
    if duplicate {
        f.prop(b"reg", &reg(0x80000000, 0x100000000));
    }
    f.prop(b"device_type", b"memory\0");
    f.end();
    f.begin(b"reserved-memory");
    if cells {
        f.cells();
    }
    f.prop(b"ranges", b"");
    f.begin(b"carveout@90000000");
    if dynamic {
        f.prop(b"size", &0x1000u64.to_be_bytes());
    } else {
        f.prop(b"reg", &reg(0x90000000, 0x100000));
    }
    f.end();
    f.end();
    f.end();
    f.finish()
}
const BOARD: &[u8] = b"nvidia,p3450-0000\0nvidia,tegra210\0";

#[test]
fn firmware_carveout_and_dtb_are_excluded_from_ram_above_and_below_four_gib() {
    let bytes = fixture(BOARD, false, true, false);
    let dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
    let memory = handoff::memory(&dtb).unwrap();
    assert_eq!(memory.ram.iter().next().unwrap().bytes(), 0x100000000);
    for start in [0x90000000, 0x83000000] {
        assert_eq!(
            admit(
                &memory.ram,
                &memory.reserved,
                Region::new(start, 4096).unwrap(),
                4096
            ),
            Err(Refusal::Reserved)
        );
    }
    assert!(
        admit(
            &memory.ram,
            &memory.reserved,
            Region::new(0x100000000, 4096).unwrap(),
            4096
        )
        .is_ok()
    );
}
#[test]
fn unknown_board_and_orin_are_not_admitted_as_original_nano() {
    for board in [
        b"nvidia,tegra210\0".as_slice(),
        b"nvidia,jetson-orin-nano\0nvidia,tegra234\0",
        b"nvidia,p3450-0000\0",
    ] {
        let bytes = fixture(board, false, true, false);
        let dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
        assert!(matches!(handoff::memory(&dtb), Err(handoff::Error::Board)));
    }
}
#[test]
fn dynamic_carveouts_missing_cells_and_duplicate_reg_refuse_boot() {
    for (dynamic, cells, duplicate) in [
        (true, true, false),
        (false, false, false),
        (false, true, true),
    ] {
        let bytes = fixture(BOARD, dynamic, cells, duplicate);
        let dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
        assert!(handoff::memory(&dtb).is_err());
    }
}

#[test]
fn exact_empty_reserved_slots_exclude_nothing_but_ram_and_nonzero_addresses_stay_strict() {
    for (start, size, accepted) in [(0, 0, true), (0x90000000, 0, false)] {
        let mut bytes = fixture(BOARD, false, true, false);
        let original = reg(0x90000000, 0x100000);
        let offset = bytes
            .windows(16)
            .position(|window| window == original)
            .unwrap();
        bytes[offset..offset + 16].copy_from_slice(&reg(start, size));
        let dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
        assert_eq!(handoff::memory(&dtb).is_ok(), accepted);
    }
    let mut bytes = fixture(BOARD, false, true, false);
    let original = reg(0x80000000, 0x100000000);
    let offset = bytes
        .windows(16)
        .position(|window| window == original)
        .unwrap();
    bytes[offset..offset + 16].fill(0);
    assert!(handoff::memory(&Dtb::parse(&bytes, 0x83000000).unwrap()).is_err());
}

fn dynamic_fixture(size: &[u8], alignment: Option<&[u8]>, ranges: Option<&[u8]>) -> Vec<u8> {
    let mut f = Fixture::new();
    f.begin(b"");
    f.cells();
    f.prop(b"compatible", BOARD);
    f.begin(b"memory@80000000");
    f.prop(b"device_type", b"memory\0");
    f.prop(b"reg", &reg(0x80000000, 0x100000000));
    f.end();
    f.begin(b"reserved-memory");
    f.cells();
    f.prop(b"ranges", b"");
    f.begin(b"vpr-carveout");
    f.prop(b"size", size);
    if let Some(alignment) = alignment {
        f.prop(b"alignment", alignment);
    }
    if let Some(ranges) = ranges {
        f.prop(b"alloc-ranges", ranges);
    }
    f.end();
    f.end();
    f.end();
    f.finish()
}

#[test]
fn dynamic_request_excludes_every_candidate_byte_instead_of_choosing_an_allocation() {
    let candidates = [reg(0x80000000, 0x70000000), reg(0x110000000, 0x10000000)].concat();
    let bytes = dynamic_fixture(
        &0x19000000u64.to_be_bytes(),
        Some(&0x400000u64.to_be_bytes()),
        Some(&candidates),
    );
    let dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
    let memory = handoff::memory(&dtb).unwrap();
    for candidate in [
        Region::new(0x80000000, 0x70000000).unwrap(),
        Region::new(0x110000000, 0x10000000).unwrap(),
    ] {
        assert!(memory.reserved.iter().any(|region| region == candidate));
        assert!(!memory.occupied.iter().any(|region| region == candidate));
    }
    for start in [0x80000000, 0x90000000, 0xeffff000, 0x110000000, 0x11ffff000] {
        assert_eq!(
            admit(
                &memory.ram,
                &memory.reserved,
                Region::new(start, 4096).unwrap(),
                4096
            ),
            Err(Refusal::Reserved)
        );
    }
    assert!(
        admit(
            &memory.ram,
            &memory.reserved,
            Region::new(0xf0080000, 4096).unwrap(),
            4096
        )
        .is_ok()
    );
}

#[test]
fn unbounded_malformed_and_impossible_dynamic_allocations_refuse() {
    let ranges = reg(0x90000001, 0x1000);
    for (size, alignment, candidates) in [
        (0x1000u64.to_be_bytes().to_vec(), None, None),
        (0u64.to_be_bytes().to_vec(), None, Some(ranges.clone())),
        (vec![1], None, Some(ranges.clone())),
        (
            0x1000u64.to_be_bytes().to_vec(),
            Some(3u64.to_be_bytes().to_vec()),
            Some(ranges.clone()),
        ),
        (
            0x1000u64.to_be_bytes().to_vec(),
            Some(0x1000u64.to_be_bytes().to_vec()),
            Some(ranges.clone()),
        ),
        (
            0x1000u64.to_be_bytes().to_vec(),
            None,
            Some(reg(u64::MAX, 0x1000)),
        ),
    ] {
        let bytes = dynamic_fixture(&size, alignment.as_deref(), candidates.as_deref());
        assert!(handoff::memory(&Dtb::parse(&bytes, 0x83000000).unwrap()).is_err());
    }
}

#[test]
fn occupied_preserves_static_regions_and_dtb_identity_without_dynamic_pools() {
    let bytes = fixture(BOARD, false, true, false);
    let mut dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
    let firmware_map = Region::new(0xff000000, 0x10000).unwrap();
    dtb.reservations.push(firmware_map, false).unwrap();
    let memory = handoff::memory(&dtb).unwrap();
    let expected = [
        firmware_map,
        Region::new(0x90000000, 0x100000).unwrap(),
        Region::new(0x83000000, bytes.len() as u64).unwrap(),
    ];
    assert_eq!(memory.occupied.iter().count(), expected.len());
    for region in expected {
        assert!(memory.occupied.iter().any(|occupied| occupied == region));
        assert!(memory.reserved.iter().any(|reserved| reserved == region));
    }
    // A larger static reservation retains its exact range; matching just an
    // inner framebuffer address is not permission to discard the reservation.
    assert!(
        !memory
            .occupied
            .iter()
            .any(|occupied| occupied == Region::new(0x90000000, 4096).unwrap())
    );
}
