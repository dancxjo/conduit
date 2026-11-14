use conduit_face::{BodyFace, FaceContext, FaceFocus, FaceNames, VisualDensity, VisualExpression, VisualUserConstraints, admit_visual_expression};
use conduitos::native_face_scene::NativeFaceScene;
fn main() {
    let body=conduit_body::Body::born("source/probe".into(), "checked/probe".into(),1,"sign/probe".into()).unwrap();
    let name="Navigation visibility probe";
    let mut expression = VisualExpression::phosphor();
    expression.density = VisualDensity::Spacious;
    let expression = admit_visual_expression(Ok(expression), VisualUserConstraints {
        forced_colors: false, reduced_motion: true,
        minimum_text_scale_percent: 150, maximum_supported_text_scale_percent: 200,
    }).unwrap();
    let mut frames = Vec::new(); let mut identities = Vec::new();
    for context in [FaceContext::Overview, FaceContext::Library] {
        let face = BodyFace::project_with_names(&body, None, 1, context, FaceFocus::Body,
            vec![], FaceNames { body_name: Some(name), resident_plots: &[] }).unwrap().face;
        identities.push(face.identity.clone());
        let scene = NativeFaceScene::prepare_visual_fallback(face, 1024, 600, true, &expression).unwrap();
        frames.push(scene.frame().unwrap().scene);
    }
    println!("different_semantic_face_ids={}", identities[0] != identities[1]);
    println!("overview_commands={}", frames[0].commands().len());
    println!("library_commands={}", frames[1].commands().len());
    println!("identical_primary_drawing={}", frames[0].commands() == frames[1].commands());
    println!("software_only=true; no_body_actions=true; no_physical_capture_claim=true");
}
