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
        while self.tree.len() % 4 != 0 {
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
