use conduit_environment_make::DisplayOutputSource;
#[test]
fn authored_output_has_explicit_frame_and_refusal_fore_and_one_electrical_sink() {
    let source = DisplayOutputSource::compile().unwrap();
    assert_eq!(
        source
            .plot
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND)
            .count(),
        1
    );
    assert_eq!(source.plot.input_bindings.len(), 1);
    assert_eq!(source.plot.output_bindings.len(), 1);
}
#[path = "display_support/planning.rs"]
mod planning;
