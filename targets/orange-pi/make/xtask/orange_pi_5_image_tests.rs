use super::*;

#[test]
fn geometry_keeps_bootloader_outside_the_fat_partition() {
    assert_eq!(BOOTLOADER_START_SECTOR, 64);
    assert!(
        BOOTLOADER_START_SECTOR * SECTOR_BYTES + U_BOOT_BYTES
            < u64::from(PARTITION_START_SECTOR) * SECTOR_BYTES
    );
    assert_eq!(IMAGE_BYTES, 64 * 1024 * 1024);
}

#[test]
fn target_is_aarch64_conduitos_and_never_loongarch() {
    assert_eq!(TARGET_ID, "conduitos/aarch64/orange-pi-5-rk3588s");
    assert!(!TARGET_ID.contains("loong"));
    assert!(!TARGET_ID.starts_with("std/"));
}

#[test]
fn kernel_lookup_matches_the_inherited_cargo_target_directory() {
    let root = Path::new("/checkout/conduit");
    let suffix = Path::new("aarch64-unknown-none/release/conduitos-aarch64-orange-pi-5");
    assert_eq!(
        cargo_kernel_path(root, None),
        root.join("target").join(suffix)
    );
    assert_eq!(
        cargo_kernel_path(root, Some(OsStr::new("build/shared"))),
        root.join("build/shared").join(suffix)
    );
    assert_eq!(
        cargo_kernel_path(root, Some(OsStr::new("/tmp/cargo-target"))),
        Path::new("/tmp/cargo-target").join(suffix)
    );
}
