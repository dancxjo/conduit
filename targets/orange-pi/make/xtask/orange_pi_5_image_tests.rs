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
