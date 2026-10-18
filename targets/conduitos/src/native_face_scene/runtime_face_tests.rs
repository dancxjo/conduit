//! Reuse genuine owner output as bounded geometry fixtures, without reenacting
//! its owner actions or treating retained native grants as live authority.
use super::*;

#[test]
fn captured_current_owner_faces_fit_without_semantic_mutation_or_detail_loss() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/audits/evidence/native-face-runtime-snapshots-20261018.json"
    ))
    .unwrap();
    let snapshots = evidence["snapshots"].as_array().unwrap();
    assert_eq!(snapshots.len(), 27);
    let mut maximum_commands = 0;
    for snapshot in snapshots {
        let face: Face = serde_json::from_value(snapshot["face"].clone()).unwrap();
        face.validate().unwrap();
        for (width, height) in [(1280, 800), (640, 480), (320, 240)] {
            for scale in [100, 200] {
                for density in [VisualDensity::Compact, VisualDensity::Spacious] {
                    let mut scene = NativeFaceScene::prepare_visual_fallback(
                        face.clone(),
                        width,
                        height,
                        true,
                        &expression(scale, density),
                    )
                    .unwrap();
                    maximum_commands = maximum_commands.max(all_pages(&mut scene));
                    scene.show_details(true);
                    maximum_commands = maximum_commands.max(all_pages(&mut scene));
                    let details: String =
                        scene.details.iter().map(|row| row.text.as_str()).collect();
                    for subject in &face.subjects {
                        assert!(
                            details.contains(&subject.name),
                            "lost original subject name"
                        );
                    }
                    for text in &face.text {
                        assert!(details.contains(&text.text), "lost original owner text");
                    }
                    assert_eq!(scene.face(), &face);
                }
            }
        }
    }
    assert!(maximum_commands > 0 && maximum_commands <= MAX_GRAPHICS_COMMANDS);
}
