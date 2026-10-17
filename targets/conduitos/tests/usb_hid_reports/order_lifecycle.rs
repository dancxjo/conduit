//! Ordering lifecycle admission through the production pure-protocol kernel.
use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
use std::sync::OnceLock;

// Source-package generation is deterministic and expensive. Reuse only these
// immutable bytes; each case still prepares its own exact Plan and fresh Play.
static PACKAGE: OnceLock<ProtocolSourcePackage> = OnceLock::new();

pub(super) fn package() -> ProtocolSourcePackage {
    PACKAGE
        .get_or_init(|| conduitos::protocol_source::usb_hid_keyboard_order_package().unwrap())
        .clone()
}

#[test]
fn ordering_lifecycle_has_exact_admitted_state_and_feedback_in_the_existing_kernel() {
    let package = package();
    PreparedProtocolSource::prepare(package.clone())
        .unwrap()
        .expand("usb-hid-keyboard-order-lifecycle")
        .unwrap();
    let (_, run) =
        super::state_kernel::prepared_package(&package, "usb-hid-keyboard-order-lifecycle");
    let boundary = &run.kernel().definition().boundary;
    assert_eq!(boundary.input_fronts.len(), 2);
    assert_eq!(boundary.output_fronts.len(), 3);
}
