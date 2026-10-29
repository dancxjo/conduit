//! Reviewed runtime target selection from OS, architecture and kernel board facts.
use std::{fs::File, io::Read, path::Path};

const B_PLUS: &str = "std/armv6/raspberry-pi-model-b-plus-v1.2";
const PI3_AARCH64: &str = "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3";
const PI5_AARCH64: &str = "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb";
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
        ("linux", "aarch64") => {
            let (compatible, revision) = observe()?;
            if pi3_plus(&compatible, &revision) {
                Ok(PI3_AARCH64)
            } else if pi5_8gb(&compatible, &revision) {
                Ok(PI5_AARCH64)
            } else {
                Err("kernel board facts do not match an installed exact 64-bit Raspberry Pi profile".into())
            }
        }
        ("linux", "arm") => {
            let (compatible, revision) = observe()?;
            if !b_plus(&compatible, &revision) {
                return Err(
                    "kernel board facts do not match reviewed Raspberry Pi Model B+ v1.2".into(),
                );
            }
            Ok(B_PLUS)
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

fn pi5_8gb(compatible: &[u8], revision: &[u8]) -> bool {
    if compatible.last() != Some(&0) || revision.len() != 4 {
        return false;
    }
    let entries: Vec<_> = compatible[..compatible.len() - 1]
        .split(|byte| *byte == 0)
        .collect();
    !entries.iter().any(|entry| entry.is_empty())
        && entries.contains(&b"raspberrypi,5-model-b".as_slice())
        && entries.contains(&b"brcm,bcm2712".as_slice())
        && u32::from_be_bytes(revision.try_into().expect("length checked")) & 0x00ff_ffff
            == 0xd04171
}

#[cfg(test)]
mod tests {
    use super::*;
    const COMPATIBLE: &[u8] = b"raspberrypi,model-b-plus\0brcm,bcm2835\0";

    #[test]
    fn aarch64_profiles_require_exact_board_and_revision() {
        for (compatible, revision, expected) in [
            (
                b"raspberrypi,3-model-b-plus\0brcm,bcm2837\0".as_slice(),
                0xa020d3u32,
                PI3_AARCH64,
            ),
            (
                b"raspberrypi,5-model-b\0brcm,bcm2712\0".as_slice(),
                0xd04171u32,
                PI5_AARCH64,
            ),
        ] {
            for warranty in [0, 0x02000000] {
                assert_eq!(
                    select_target("linux", "aarch64", || Ok((
                        compatible.to_vec(),
                        (revision | warranty).to_be_bytes().to_vec()
                    )))
                    .unwrap(),
                    expected
                );
            }
            assert!(select_target("linux", "aarch64", || Ok((
                compatible.to_vec(),
                0u32.to_be_bytes().to_vec()
            )))
            .is_err());
        }
        for compatible in [
            b"raspberrypi,5-model-b\0brcm,bcm2712\0".as_slice(),
            b"raspberrypi,3-model-b-plus\0brcm,bcm2837\0".as_slice(),
        ] {
            for revision in [vec![], vec![0], vec![0, 0, 0], vec![0, 0, 0, 0, 0]] {
                assert!(
                    select_target("linux", "aarch64", || Ok((compatible.to_vec(), revision)))
                        .is_err()
                );
            }
        }
        assert!(select_target("linux", "aarch64", || Err("missing device tree".into())).is_err());
        assert!(!pi5_8gb(
            b"raspberrypi,5-model-b\0brcm,bcm2712",
            &0xd04171u32.to_be_bytes()
        ));
        assert!(!pi5_8gb(
            b"raspberrypi,5-model-b\0brcm,bcm2837\0",
            &0xd04171u32.to_be_bytes()
        ));
        assert!(!pi5_8gb(
            b"raspberrypi,5-model-b\0brcm,bcm2712\0",
            &0xc04170u32.to_be_bytes()
        ));
    }

    #[test]
    fn aarch64_profiles_check_against_current_workspace_catalog() {
        for (source, expected) in [
            (include_str!("../../../../targets/std/profiles/raspberry-pi-3-model-b-plus-rev-1.3-aarch64-linux.host.conduit"), PI3_AARCH64),
            (include_str!("../../../../targets/std/profiles/raspberry-pi-5-model-b-rev-1.1-8gb-aarch64-linux.host.conduit"), PI5_AARCH64),
        ] {
            let configuration = conduit_host_make::parse_host_configuration_conduit(source).unwrap();
            let checked = conduit_host_make::check_host_configuration(configuration, &conduit_workspace_make::catalog(), &conduit_workspace_make::package_set()).unwrap();
            assert_eq!(checked.profile().target.key(), expected);
        }
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
        assert!(select_target("linux", "aarch64", || Ok((
            COMPATIBLE.to_vec(),
            0x10u32.to_be_bytes().to_vec()
        )))
        .is_err());
    }
}
