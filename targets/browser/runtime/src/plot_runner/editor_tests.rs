use super::*;
const SOURCE: &str = include_str!("../../../../../plots/memory-lantern/main.conduit");

fn request(effect: TourHostEffect) -> Box<TourKeyEventEffect> {
    let TourHostEffect::ApplicationEvent(effect) = effect else {
        panic!("expected DOM editor request")
    };
    assert_eq!(effect.effect_kind, "text-editor");
    effect
}
fn next(session: &mut TourSession) -> Box<TourKeyEventEffect> {
    for _ in 0..crate::installed_browser::BROWSER_ROUTE_SLOTS {
        if let Some(index) = session
            .pending
            .iter()
            .position(|pending| matches!(pending.effect, engine::BrowserHostEffect::TextEditor(_)))
        {
            return request(session.project_pending_effect(index).unwrap());
        }
        session.poll_effect().unwrap();
    }
    panic!("next authoritative document")
}
fn complete(session: &mut TourSession, effect: &TourKeyEventEffect, bytes: &[u8]) {
    session
        .complete_effect(
            &effect.active_play_id,
            &effect.placement_id,
            effect.request_sequence,
            Some(bytes),
        )
        .unwrap();
}
fn edit(revision: u64, start: u32, end: u32, text: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&revision.to_le_bytes());
    bytes.extend_from_slice(&start.to_le_bytes());
    bytes.extend_from_slice(&end.to_le_bytes());
    bytes.extend_from_slice(text);
    bytes
}
fn snapshot(session: &TourSession) -> (u64, Vec<u8>) {
    let index = session.fragments[0]
        .placements
        .iter()
        .position(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND)
        .unwrap();
    let (revision, text) = session
        .scheduler
        .retained_text_snapshot(conduit_kernel::NodeId(index as u16));
    (revision, text.to_vec())
}

#[test]
fn dom_editor_loop_commits_unicode_through_the_scheduler() {
    let (mut session, effect) =
        TourSession::prepare("browser/editor-proof", "boot/editor-proof", SOURCE, 1).unwrap();
    let effect = request(effect);
    assert_eq!(effect.editor.as_ref().unwrap().text, "");
    assert_eq!(
        effect.editor.as_ref().unwrap().label.as_deref(),
        Some("Message")
    );
    complete(&mut session, &effect, &edit(0, 0, 0, "café".as_bytes()));
    let next = next(&mut session);
    let document = next.editor.as_ref().unwrap();
    assert_eq!(document.revision, "1");
    assert_eq!(document.text.as_bytes(), "café".as_bytes());
    assert_eq!(snapshot(&session), (1, "café".as_bytes().to_vec()));
    // An old effect completion cannot exercise the newly pending request.
    assert!(session
        .complete_effect(
            &effect.active_play_id,
            &effect.placement_id,
            effect.request_sequence,
            Some(&edit(1, 0, 0, b"forged"))
        )
        .is_err());
    assert_eq!(snapshot(&session), (1, "café".as_bytes().to_vec()));
    let receipt = session.cancel().unwrap();
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.active_play_id, next.active_play_id);
}

#[test]
fn invalid_dom_edits_refuse_distinctly_without_mutating_authoritative_state() {
    for (bytes, code, detail) in [
        (edit(7, 0, 0, b"stale"), "InvalidInput", 12),
        (edit(1, 4, 5, b"range"), "InvalidInput", 13),
        (edit(1, 0, 0, &[0xff]), "InvalidInput", 14),
        (vec![0], "InvalidInput", 15),
        (edit(1, 0, 0, &[b'x'; 256]), "StateCapacityExhausted", 10),
    ] {
        let (mut session, effect) =
            TourSession::prepare("browser/editor-refusal", "boot/editor-refusal", SOURCE, 1)
                .unwrap();
        complete(
            &mut session,
            &request(effect),
            &edit(0, 0, 0, "café".as_bytes()),
        );
        let effect = next(&mut session);
        let error = match session.complete_effect(
            &effect.active_play_id,
            &effect.placement_id,
            effect.request_sequence,
            Some(&bytes),
        ) {
            Err(error) => error,
            Ok(_) => panic!("invalid edit must refuse"),
        };
        assert_eq!(
            error,
            format!("BackFailed(Failure {{ code: {code}, detail: {detail} }})")
        );
        assert_eq!(snapshot(&session), (1, "café".as_bytes().to_vec()));
    }
}

#[test]
fn oversized_editor_completions_preserve_storage_and_pending_request() {
    use conduit_kernel::ValueStorage;
    let source = SOURCE.replace(
        "    message:",
        "    pointer: input/pointer-source\n    message:",
    );
    let (mut session, _) =
        TourSession::prepare("browser/editor-bound", "boot/editor-bound", &source, 1).unwrap();
    let effect = next(&mut session);
    let oversized = vec![b'x'; conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES as usize + 1];
    // Establish that the larger mixed-Plot slot really accepts this payload.
    let probe = session.scheduler.store_host_value(&oversized).unwrap();
    session.scheduler.discard_host_value(probe).unwrap();
    let before = (
        session.scheduler.values().used_items(),
        session.scheduler.values().used_bytes(),
    );
    let pending_count = session.pending.len();
    for _ in 0..64 {
        let error = session
            .complete_effect(
                &effect.active_play_id,
                &effect.placement_id,
                effect.request_sequence,
                Some(&oversized),
            )
            .unwrap_err();
        assert_eq!(error, "browser input exceeded its planned bound");
        assert_eq!(
            (
                session.scheduler.values().used_items(),
                session.scheduler.values().used_bytes()
            ),
            before
        );
        assert_eq!(session.pending.len(), pending_count);
        assert_eq!(snapshot(&session), (0, Vec::new()));
    }
    complete(&mut session, &effect, &edit(0, 0, 0, "café".as_bytes()));
    let document = next(&mut session);
    assert_eq!(document.editor.as_ref().unwrap().text, "café");
    assert_eq!(snapshot(&session), (1, "café".as_bytes().to_vec()));
}
