use super::super::memory::{Refusal, admit};
use super::*;
use alloc::vec::Vec;

#[test]
fn parses_stock_style_values_and_excludes_both_allocations() {
    let parsed =
        bootargs(b"console=ttyS0 lp0_vec=0x1000@0xff780000 tegra_fbmem=0x800000@0x92cb6000\0")
            .unwrap();
    assert_eq!(
        parsed.framebuffer,
        Some(Region::new(0x92cb6000, 0x800000).unwrap())
    );
    assert_eq!(
        parsed.lp0_vector,
        Some(Region::new(0xff780000, 4096).unwrap())
    );
    let mut ram = Regions::new();
    ram.push(Region::new(0x80000000, 0x100000000).unwrap(), true)
        .unwrap();
    let mut excluded = Regions::new();
    parsed.append_to(&mut excluded).unwrap();
    for start in [0x92cb6000, 0xff780000] {
        assert_eq!(
            admit(&ram, &excluded, Region::new(start, 4096).unwrap(), 4096),
            Err(Refusal::Reserved)
        );
    }
}

#[test]
fn numbers_are_complete_bounded_and_unambiguous() {
    for (text, expected) in [
        (b"8192".as_slice(), 8192),
        (b"8M", 8388608),
        (b"0x10k", 16384),
        (b"1G", 1073741824),
    ] {
        assert_eq!(number(text), Ok(expected));
    }
    for text in [
        b"".as_slice(),
        b"0x",
        b"020",
        b"-1",
        b"1junk",
        b"18446744073709551616",
        b"0xffffffffffffffffK",
        b"12@13",
    ] {
        assert!(number(text).is_err(), "{text:?}");
    }
}

#[test]
fn malformed_or_duplicate_known_parameters_never_disappear() {
    for text in [
        b"tegra_fbmem\0".as_slice(),
        b"tegra_fbmem=\0",
        b"lp0_vec=1\0",
        b"lp0_vec=0@0\0",
        b"lp0_vec=2@0xffffffffffffffff\0",
        b"lp0_vec=1@2@3\0",
        b"lp0_vec=1@2 lp0_vec=1@2\0",
        b"tegra_fbmem=1@2 tegra_fbmem=1@2\0",
    ] {
        assert!(bootargs(text).is_err(), "{text:?}");
    }
    assert!(bootargs(b"tegra_fbmem=1@2").is_err());
    assert!(bootargs(b"\"tegra_fbmem=1@2\"\0").is_err());
    assert!(bootargs(b"x=1\0tegra_fbmem=1@2\0").is_err());
    assert!(bootargs(&vec![b' '; MAX_BOOTARGS + 1]).is_err());
}

#[test]
fn absence_and_unknown_parameters_do_not_invent_allocations() {
    let parsed = bootargs(b"root=/dev/mmcblk0p1 vpr_resize console=tty0\0").unwrap();
    assert_eq!(parsed.framebuffer, None);
    assert_eq!(parsed.lp0_vector, None);
}

fn tree(arguments: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(1u32.to_be_bytes());
    bytes.extend([0; 4]);
    bytes.extend(1u32.to_be_bytes());
    bytes.extend(b"chosen\0\0");
    for argument in arguments {
        for word in [3, argument.len() as u32, 0] {
            bytes.extend(word.to_be_bytes());
        }
        bytes.extend(*argument);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
    }
    for word in [2u32, 2, 9] {
        bytes.extend(word.to_be_bytes());
    }
    bytes
}

#[test]
fn chosen_bootargs_is_extracted_once_and_duplicate_property_refuses() {
    for (arguments, valid) in [
        (vec![b"lp0_vec=4K@0xff780000\0".as_slice()], true),
        (
            vec![b"lp0_vec=4K@0xff780000\0".as_slice(), b"console=tty0\0"],
            false,
        ),
    ] {
        let structure = tree(&arguments);
        let dtb = Dtb {
            structure: &structure,
            strings: b"bootargs\0",
            reservations: Regions::new(),
            total_bytes: structure.len(),
        };
        let parsed = from_dtb(&dtb);
        if valid {
            assert_eq!(
                parsed.unwrap().lp0_vector,
                Some(Region::new(0xff780000, 4096).unwrap())
            );
        } else {
            assert_eq!(parsed.unwrap_err(), Error::Duplicate);
        }
    }
}
