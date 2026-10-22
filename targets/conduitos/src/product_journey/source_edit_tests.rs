//! Owner-authorized Source handoff over the actual resident Plan/Play.
use super::*;

#[test]
fn patchbay_source_handoff_requires_one_current_owner_request_and_preserves_drafts() {
    let (ids, offer, mut journey) = born();
    run(&mut journey, &ids, &offer);
    select(&mut journey, NativePlot::KeyboardCanvas);
    select(&mut journey, NativePlot::Patchbay);
    let graph = journey.foreground_patchbay_graph().unwrap();
    let forged = patchbay_application::PatchbayApplicationRequest::EditCurrent {
        expanded_plot_id: graph.expanded_plot_id.clone(),
        subject_identity: graph.subject_identities().next().unwrap().into(),
    };
    assert!(matches!(
        journey.open_patchbay_source(forged, &ids),
        Err(SourceEditError::Refused(SourceEditRefusal::WrongTarget))
    ));
    let request = |journey: &mut ProductJourney| {
        let subject = journey
            .foreground_patchbay_graph()
            .unwrap()
            .subject_identities()
            .next()
            .unwrap()
            .to_owned();
        super::test_support::submit_patchbay(
            journey,
            patchbay_application::EDIT_CURRENT_ACTION_ID,
            Some(&subject),
        );
        match journey.take_application_request().unwrap() {
            native_workset::NativeApplicationRequest::EditCurrent(request) => request,
            _ => panic!("expected actual owner edit request"),
        }
    };
    let issued = request(&mut journey);
    let plan = journey.current_plan().unwrap().plan_id.clone();
    let play = journey.current_play().unwrap().clone();
    let receipt = journey.open_patchbay_source(issued.clone(), &ids).unwrap();
    assert_eq!(
        receipt.source,
        native_workset::resident(NativePlot::KeyboardCanvas).unwrap()
    );
    assert_eq!(receipt.body_plan_id, plan);
    assert!(!receipt.selection_enacted);
    assert_eq!(
        journey.projection().result.as_deref(),
        Some(NativePlot::KeyboardCanvas.source())
    );
    assert!(journey.foreground_text_editor_active());
    assert_eq!(journey.current_plan().unwrap().plan_id, plan);
    assert_eq!(journey.current_play().unwrap(), &play);
    assert!(matches!(
        journey.open_patchbay_source(issued, &ids),
        Err(SourceEditError::Refused(SourceEditRefusal::WrongTarget))
    ));
    select(&mut journey, NativePlot::KeyboardCanvas);
    select(&mut journey, NativePlot::Patchbay);
    let issued = request(&mut journey);
    assert!(matches!(
        journey.open_patchbay_source(issued, &ids),
        Err(SourceEditError::Refused(SourceEditRefusal::ExistingDraft))
    ));
    assert!(journey.take_application_request().is_none());
    assert_eq!(journey.current_play().unwrap(), &play);
    select(&mut journey, NativePlot::MemoryLantern);
    assert_eq!(
        journey.projection().result.as_deref(),
        Some(NativePlot::KeyboardCanvas.source())
    );
    select(&mut journey, NativePlot::KeyboardCanvas);
    select(&mut journey, NativePlot::Patchbay);
    let stale = request(&mut journey);
    select(&mut journey, NativePlot::Tour);
    assert!(matches!(
        journey.open_patchbay_source(stale, &ids),
        Err(SourceEditError::Refused(SourceEditRefusal::WrongOwner))
    ));
    assert!(journey.take_application_request().is_none());
    assert_eq!(journey.current_play().unwrap(), &play);
}
