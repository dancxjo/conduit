//! Spoken Face Language admission proof, before any playback effect.
use super::*;

#[test]
fn english_proof_face_back_refuses_french_before_selected_playback() {
    let (face, show) = source(1);
    let (_, batch) = batch(&face, &show);
    let (config, selection, authorization) = selected_fake_playback(FakePlaybackBehavior::Success);
    let mut host = StdHost::new_with_playback(
        config.clone(),
        StdHostComposition::minimal().with_text(),
        selection.clone(),
    )
    .unwrap();
    let before = host.advertisement().clone();
    let result = super::playback::run_selected_spoken_playback(
        &face,
        &show,
        &batch,
        &crate::hosted_language::tests::request("language/french"),
        &"00".repeat(32),
        config,
        selection,
        &authorization,
        &crate::RunControl::default(),
        &mut host,
        false,
    );
    let Err(SpokenStreamExecutionRefusal::Plan(detail)) = result else {
        panic!("French must be refused during coverage planning");
    };
    assert!(detail.contains("LanguageCoverageUnsatisfied"), "{detail}");
    assert!(detail.contains("language/french"), "{detail}");
    assert_eq!(host.advertisement(), &before);
    assert!(host.kernel_resources.is_idle());
}
