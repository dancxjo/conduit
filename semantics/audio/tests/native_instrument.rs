use conduit_audio::{
    InstrumentControl, InstrumentMapping, InstrumentPitchMillihertz,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn instrument_mapping_round_trips_through_its_authored_type() {
    let pitches = InstrumentPitchMillihertz::new([
        261_626, 293_665, 329_628, 349_228, 391_995, 440_000, 493_883, 523_251,
    ])
    .unwrap();
    let mapping = InstrumentMapping::new(1, 0, pitches, 8).unwrap();
    let structured = mapping.clone().into_structured().unwrap();
    assert_eq!(InstrumentMapping::from_structured(structured).unwrap(), mapping);
}

#[test]
fn both_instrument_control_payloads_are_native() {
    let controls = [
        InstrumentControl::analog(10, 1, 500_000).unwrap(),
        InstrumentControl::button(true, 20, 2, 3).unwrap(),
    ];
    for control in controls {
        let structured = control.clone().into_structured().unwrap();
        assert_eq!(InstrumentControl::from_structured(structured).unwrap(), control);
    }
}
