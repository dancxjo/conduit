//! Serialization bounds for the declared source specimen, not installed Plots.
use super::*;
use crate::BirthPlotChoice;
use conduit_body::ResidentPlot;

#[test]
fn finite_collection_specimen_has_a_measured_bounded_input_envelope() {
    extern crate std;
    for (items, selected) in [(1, false), (1, true), (16, false), (16, true)] {
        let choices = (0..items)
            .map(|index| BirthPlotChoice {
                title: format!("{}!", "界".repeat(21)),
                search_text: "finite source fixture".into(),
                plot: ResidentPlot::new(
                    format!("{index:064x}").into(),
                    format!("{index:064x}").into(),
                ),
                refusal: None,
                selected,
            })
            .collect();
        let mut draft =
            BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
        draft.edit_name(draft.revision(), "é".repeat(32)).unwrap();
        let mut context = basis();
        context.encounter_id = "e".repeat(128);
        let face = draft.face(&context).unwrap();
        let bytes = serde_json::to_vec(&face).unwrap();
        std::println!(
            "collection-profile items={items} selected={selected} face_json_bytes={} subjects={} properties={}",
            bytes.len(),
            face.subjects.len(),
            face.properties.len()
        );
        assert!(
            bytes.len() <= 256 * 1024,
            "source specimen input bytes={}",
            bytes.len()
        );
        assert_eq!(draft.choices().len(), items);
    }
}
