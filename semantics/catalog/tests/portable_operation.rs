#![cfg(feature = "kernel-step")]
//! A non-environmental authored computation uses shared preparation directly.
use conduit_core::{ConfigurationValue, PortTemporal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    PortableOperationBack, PortableOperationRefusal, PreparedPortableOperation,
};

fn gear() -> conduit_plot::CheckedGear {
    let checked = check_syntax_document(
        &parse_syntax_document("plot increment (\n input: U8 >> result: U8\n) = (. + 1)\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    expand_canonical_plot_for_authoring(&checked, "increment", &ProfileCatalog::new())
        .unwrap()
        .expanded
        .gears
        .remove(0)
}
#[test]
fn authored_increment_prepares_and_runs_without_application_defaults() {
    let prepared =
        PreparedPortableOperation::prepare(&gear(), PortTemporal::Flow { closes: true }, 16, 16)
            .unwrap();
    assert!(prepared.host_calls.is_empty());
    assert!(prepared.selector.is_none());
    let PortableOperationBack::Expression(mut back) = prepared.back else {
        panic!("expression")
    };
    let mut io = StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 1,
        })],
        [false],
        [Some(16)],
        None,
        16,
    );
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&[41])], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        <conduit_semantic_catalog::PreparedExpressionBack as StepBack<1>>::prepared_output(
            &back,
            PortId(0)
        ),
        Some(&[42][..])
    );
}
#[test]
fn missing_or_malformed_configuration_refuses() {
    let mut gear = gear();
    gear.configuration.clear();
    assert!(matches!(
        PreparedPortableOperation::prepare(&gear, PortTemporal::Flow { closes: true }, 16, 16),
        Err(PortableOperationRefusal::Unsupported)
    ));
    gear.configuration.push(conduit_core::ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text("malformed".into()),
    });
    assert!(matches!(
        PreparedPortableOperation::prepare(&gear, PortTemporal::Flow { closes: true }, 16, 16),
        Err(PortableOperationRefusal::Initialization)
    ));
}
