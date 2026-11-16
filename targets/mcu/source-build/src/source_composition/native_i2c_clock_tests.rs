//! Compile the actual emitted constructor against original native APIs. The
//! inventory test uses calibrated (permuted) kernel nodes, not Source ordinals.
mod layout {
    pub const SOURCE_DOCUMENT_ID: &str = "test/source";
    pub const CHECKED_PLOT_ID: &str = "test/checked";
    pub const EXPANDED_PLOT_ID: &str = "test/expanded";
}
mod source {
    pub static NATIVE_KINDS: &[(u16, &str)] =
        &[(19, "machine/clock/at"), (7, "machine/i2c/transact")];
}
include!("native_i2c_clock.rs");

#[test]
fn exact_native_pair_uses_remapped_kernel_nodes() {
    assert_eq!(selected_nodes(source::NATIVE_KINDS), Ok((7, 19)));
}
#[test]
fn mixed_duplicate_missing_inventory_refuses_before_native_preparation() {
    for inventory in [
        &[][..],
        &[(1, I2C_KIND)][..],
        &[(1, I2C_KIND), (2, I2C_KIND)][..],
        &[(1, I2C_KIND), (1, CLOCK_KIND)][..],
        &[(1, I2C_KIND), (2, CLOCK_KIND), (3, "machine/pulse/read")][..],
    ] {
        assert_eq!(selected_nodes(inventory), Err("native-source-inventory"));
    }
}
