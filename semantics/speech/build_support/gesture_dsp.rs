//! Native custody bindings and exact programs for the existing frame graph.
use conduit_plot::{CheckedSyntaxDocument, ProfileCatalog};
use std::{env, fs, path::PathBuf};
pub fn write(source: &CheckedSyntaxDocument) {
    let names = [
        "SpeechGestureControlledFrame",
        "SpeechGestureFrameGates",
        "SpeechFrameInput",
        "SpeechFrameCycleControl",
        "SpeechCycleControlMode",
        "SpeechAcousticTarget",
        "SpeechFrameState",
        "SpeechFrameResult",
        "SpeechStart",
    ];
    let types = source
        .native_types
        .iter()
        .filter(|t| names.contains(&t.name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(types.len(), names.len());
    let bindings = conduit_plot::rust_binding::generate_rust_bindings(
        &types,
        &conduit_plot::rust_binding::RustBindingOptions::default(),
    )
    .unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("gesture_dsp_types.rs"), bindings.source).unwrap();
    let plot = conduit_plot::expand_canonical_plot_for_authoring(
        source,
        "speech/gesture-controlled-frame",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let programs = plot
        .expanded
        .gears
        .iter()
        .map(|gear| {
            let conduit_core::ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
                panic!("expression")
            };
            hex.clone()
        })
        .collect::<Vec<_>>();
    let input_bindings = plot
        .input_bindings
        .iter()
        .map(|b| {
            plot.expanded
                .gears
                .iter()
                .position(|g| g.gear_id == b.gear_id)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let connections = plot
        .expanded
        .connections
        .iter()
        .map(|c| {
            (
                plot.expanded
                    .gears
                    .iter()
                    .position(|g| g.gear_id == c.source_gear_id)
                    .unwrap(),
                plot.expanded
                    .gears
                    .iter()
                    .position(|g| g.gear_id == c.sink_gear_id)
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let initial = conduit_plot::expand_canonical_plot_for_authoring(
        source,
        "speech/initial-state",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let conduit_core::ConfigurationValue::Text(initial_hex) =
        &initial.expanded.gears[0].configuration[0].value
    else {
        panic!("initial Source")
    };
    let result = plot
        .expanded
        .gears
        .iter()
        .position(|g| g.gear_id == plot.output_bindings[0].gear_id)
        .unwrap();
    fs::write(out.join("gesture_dsp_programs.rs"),format!("pub const PROGRAMS:&[&str]=&{programs:?};\npub const INPUTS:&[usize]=&{input_bindings:?};\npub const CONNECTIONS:&[(usize,usize)]=&{connections:?};\npub const RESULT:usize={result};\npub const INITIAL_STATE:&str={initial_hex:?};\n")).unwrap();
}
