//! Checked HID class interpretation; no endpoint or device admission claim.
#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;

mod usb_hid_reports {
    mod batches;
    pub(super) mod common;
    mod endpoint;
    mod endpoint_kernel;
    mod kernel;
    mod kernel_fixture;
    mod keyboard;
    mod lifecycle;
    mod mouse;
    mod ordering;
    mod pressure;
    mod received_kernel;
    mod state_kernel;
    mod transitions;
}

#[test]
fn every_hid_report_plot_checks_and_expands() {
    use conduit_plot::*;
    let syntax = parse_syntax_document(usb_hid_reports::common::SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let prepared = conduitos::protocol_source::PreparedProtocolSource::prepare(
        conduitos::protocol_source::ProtocolSourcePackage::compile(
            usb_hid_reports::common::SOURCE.into(),
            &[],
        )
        .unwrap(),
    )
    .unwrap();
    for plot in &checked.plots {
        prepared
            .expand(&plot.name)
            .unwrap_or_else(|error| panic!("{}: {error:?}", plot.name));
    }
}
