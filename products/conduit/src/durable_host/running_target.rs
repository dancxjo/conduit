//! Reviewed runtime target selection from OS, architecture and kernel board facts.
use std::{fs::File, io::Read, path::Path};

const B_PLUS: &str = "std/armv6/raspberry-pi-model-b-plus-v1.2";
const PI3_PLUS: &str = "std/armv6/raspberry-pi-3-model-b-plus-rev-1.3";
const DEVICE_TREE: &str = "/sys/firmware/devicetree/base";

pub(super) fn running_target_id() -> Result<&'static str, String> {
    select_target(std::env::consts::OS, std::env::consts::ARCH, || {
        let root = Path::new(DEVICE_TREE);
        Ok((
            read_property(&root.join("compatible"), 1024)?,
            read_property(&root.join("system/linux,revision"), 4)?,
        ))
    })
}

fn read_property(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("observe kernel board property {}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(format!(
            "kernel board property {} violates its bound",
            path.display()
        ));
    }
    Ok(bytes)
}

fn select_target(
    os: &str,
    architecture: &str,
    observe: impl FnOnce() -> Result<(Vec<u8>, Vec<u8>), String>,
) -> Result<&'static str, String> {
    match (os, architecture) {
        ("linux", "x86_64") => Ok("std/x86_64/computer"),
        ("windows", "x86_64") => Ok("std/x86_64/windows-computer"),
        ("macos", "aarch64") => Ok("std/aarch64/macos-computer"),
        ("linux", "arm") => {
            let (compatible, revision) = observe()?;
            if b_plus(&compatible, &revision) {
                Ok(B_PLUS)
            } else if pi3_plus(&compatible, &revision) {
                Ok(PI3_PLUS)
            } else {
                Err("kernel board facts do not match an installed exact 32-bit Raspberry Pi profile".into())
            }
        }
        _ => Err(format!(
            "no reviewed durable host profile is installed for {os}/{architecture}"
        )),
    }
}

fn b_plus(compatible: &[u8], revision: &[u8]) -> bool {
    // Linux exports DT strings as NUL-terminated lists and u32 properties in
    // big-endian order. Neither generic ARM nor a BCM2835 cpuinfo label proves B+.
    if compatible.last() != Some(&0) || revision.len() != 4 {
        return false;
    }
    let entries: Vec<_> = compatible[..compatible.len() - 1]
        .split(|byte| *byte == 0)
        .collect();
    if entries.iter().any(|entry| entry.is_empty())
        || !entries.contains(&b"raspberrypi,model-b-plus".as_slice())
        || !entries.contains(&b"brcm,bcm2835".as_slice())
    {
        return false;
    }
    // Exact published B+ v1.2 codes. Upper policy/warranty bits do not identify
    // the PCB. https://www.raspberrypi.com/documentation/computers/raspberry-pi.html
    let revision = u32::from_be_bytes(revision.try_into().expect("length checked")) & 0x00ff_ffff;
    matches!(revision, 0x0010 | 0x0013 | 0x900032)
}

fn pi3_plus(compatible: &[u8], revision: &[u8]) -> bool {
    if compatible.last() != Some(&0) || revision.len() != 4 {
        return false;
    }
    let entries: Vec<_> = compatible[..compatible.len() - 1]
        .split(|byte| *byte == 0)
        .collect();
    !entries.iter().any(|entry| entry.is_empty())
        && entries.contains(&b"raspberrypi,3-model-b-plus".as_slice())
        && entries.contains(&b"brcm,bcm2837".as_slice())
        && u32::from_be_bytes(revision.try_into().expect("length checked")) & 0x00ff_ffff
            == 0xa020d3
}

#[cfg(test)]
mod tests {
    use super::*;
    const COMPATIBLE: &[u8] = b"raspberrypi,model-b-plus\0brcm,bcm2835\0";

    #[test]
    fn pi3_plus_32bit_profile_requires_exact_kernel_board_facts() {
        let compatible = b"raspberrypi,3-model-b-plus\0brcm,bcm2837\0";
        for revision in [0xa020d3u32, 0x02a020d3] {
            assert_eq!(
                select_target("linux", "arm", || Ok((
                    compatible.to_vec(),
                    revision.to_be_bytes().to_vec()
                )))
                .unwrap(),
                PI3_PLUS
            );
        }
        for revision in [0x10u32, 0xa02082, 0xa220a0, 0] {
            assert!(!pi3_plus(compatible, &revision.to_be_bytes()));
        }
        assert!(!pi3_plus(COMPATIBLE, &0xa020d3u32.to_be_bytes()));
        assert!(!pi3_plus(
            b"raspberrypi,3-model-b-plus\0brcm,bcm2837",
            &0xa020d3u32.to_be_bytes()
        ));
        assert!(!pi3_plus(compatible, &[0xa0, 0x20, 0xd3]));
    }

    #[test]
    fn running_target_accepts_exact_b_plus_kernel_facts() {
        for revision in [0x10u32, 0x13, 0x900032, 0x02900032] {
            assert_eq!(
                select_target("linux", "arm", || Ok((
                    COMPATIBLE.to_vec(),
                    revision.to_be_bytes().to_vec()
                )))
                .unwrap(),
                B_PLUS
            );
        }
    }

    #[test]
    fn running_target_refuses_other_boards_missing_and_malformed_facts() {
        for revision in [0u32, 0x11, 0x900092, 0xa020d3, 0x900031] {
            assert!(!b_plus(COMPATIBLE, &revision.to_be_bytes()));
        }
        for compatible in [
            b"raspberrypi,3-model-b-plus\0brcm,bcm2837\0".as_slice(),
            b"brcm,bcm2835\0",
            b"raspberrypi,model-b-plus\0brcm,bcm2835",
            b"raspberrypi,model-b-plus\0\0brcm,bcm2835\0",
        ] {
            assert!(!b_plus(compatible, &0x10u32.to_be_bytes()));
        }
        assert!(!b_plus(COMPATIBLE, &[0, 0, 0]));
        assert!(!b_plus(COMPATIBLE, &[0x10, 0, 0, 0]));
        assert!(select_target("linux", "arm", || Err("missing DT".into())).is_err());
    }

    #[test]
    fn running_target_preserves_existing_platforms_without_reading_board() {
        for (os, arch, expected) in [
            ("linux", "x86_64", "std/x86_64/computer"),
            ("windows", "x86_64", "std/x86_64/windows-computer"),
            ("macos", "aarch64", "std/aarch64/macos-computer"),
        ] {
            assert_eq!(
                select_target(os, arch, || panic!("unneeded board read")).unwrap(),
                expected
            );
        }
        assert!(select_target("linux", "aarch64", || panic!("unsupported platform")).is_err());
    }
}
