use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{ChordInfo, ControlChordModifier};

fn assert_copy<T: Copy + Ord + core::hash::Hash>() {}

#[test]
fn every_native_chord_meaning_round_trips_without_redundant_fields() {
    assert_copy::<ChordInfo>();
    let controls = [
        ControlChordModifier::Left,
        ControlChordModifier::Right,
        ControlChordModifier::Both,
    ];
    let mut values = Vec::new();
    for modifier in controls {
        values.push(ChordInfo::CancelOrEscape(modifier));
        values.push(ChordInfo::ClearOrRefresh(modifier));
        values.push(ChordInfo::RepeatOrReplan(modifier));
    }
    values.extend([
        ChordInfo::Palette,
        ChordInfo::Inspect,
        ChordInfo::Plan,
        ChordInfo::Command,
        ChordInfo::Activate,
    ]);

    for value in values {
        let structured = value.into_structured().unwrap();
        assert_eq!(ChordInfo::from_structured(structured).unwrap(), value);
        assert_eq!(ChordInfo::decode(&value.encode()).unwrap(), value);
    }
}

#[test]
fn chord_info_has_no_handwritten_record_duplicate() {
    assert!(!include_str!("../src/input_chord.rs").contains("pub struct ChordInfo"));
}
