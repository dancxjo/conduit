use super::*;

#[test]
fn invalid_replacement_preserves_the_original_play_and_pending_effect() {
    let source = "form original {\n message: text/literal(\"hello\")\n result: presentation/text\n message > result\n}\n";
    let (session, _) = TourSession::prepare("h", "b", source, 1).unwrap();
    let play = session.active_play_id.clone();
    let fragment = session.fragments[0].clone();
    let pending = session.pending[0].request;
    SESSION.with(|slot| *slot.borrow_mut() = Some(session));

    for (admitted, proposed, expected) in [
        ("one", "two", ERROR_INTERACTION),
        ("not a Form", "not a Form", ERROR_PREPARE),
    ] {
        INPUT.with(|input| {
            input.borrow_mut()[..admitted.len()].copy_from_slice(admitted.as_bytes())
        });
        assert_eq!(
            conduit_tour_admit_source_interaction(admitted.len(), 2),
            STATUS_READY
        );
        INPUT.with(|input| {
            let mut input = input.borrow_mut();
            input[..2].copy_from_slice(b"hb");
            input[2..2 + proposed.len()].copy_from_slice(proposed.as_bytes());
        });
        assert_eq!(conduit_tour_start(1, 1, proposed.len(), 2), expected);
        SESSION.with(|slot| {
            let slot = slot.borrow();
            let current = slot.as_ref().expect("refusal preserves current session");
            assert_eq!(current.active_play_id, play);
            assert_eq!(
                current.fragments.as_slice(),
                core::slice::from_ref(&fragment)
            );
            assert_eq!(current.pending.len(), 1);
            assert_eq!(current.pending[0].request, pending);
        });
    }
    assert_eq!(conduit_tour_complete(), STATUS_READY);
    SESSION.with(|slot| assert!(slot.borrow().is_none()));
}
