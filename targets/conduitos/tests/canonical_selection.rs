//! Root selection indices must bind only the actual finite prepared inventory.
#[path = "../src/protected_application/canonical_selection.rs"]
mod selection;
use selection::selection_target;

#[test]
fn sparse_inventory_preserves_native_targets_and_refuses_invented_indices() {
    let inventory = [None, Some("keyboard"), None, Some("lantern")];
    assert_eq!(selection_target(&inventory, "patchbay.plot.0"), Some(1));
    assert_eq!(selection_target(&inventory, "patchbay.plot.1"), Some(3));
    for forged in [
        "patchbay.plot.2",
        "patchbay.plot.4",
        "patchbay.plot.-1",
        "patchbay.plot.+1",
        "patchbay.plot.01",
        "patchbay.plot. 1",
        "patchbay.plot.1 ",
        "patchbay.plot.",
        "patchbay.plot.184467440737095516160",
        "patchbay.edit-current",
    ] {
        assert_eq!(selection_target(&inventory, forged), None, "{forged}");
    }
    assert_eq!(selection_target(&[None::<()>; 4], "patchbay.plot.0"), None);
}
