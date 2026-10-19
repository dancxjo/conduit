//! Cursor scanout preserves the current semantic Show and owner state.
use super::*;

#[test]
fn motion_draws_cursor_without_submitting_or_republishing_the_face() {
    let (mut mask, draft) = fixture();
    let basis = mask.birth_basis("arrival/motion");
    let face = draft.face(&basis).unwrap();
    let mut display = CountingDisplay {
        writes: 0,
        fail: false,
    };
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    let show = mask.show().unwrap().clone();
    let focus = mask.scene().unwrap().focused();
    let writes = display.writes;
    mask.pointer_motion(100, 100, &mut display).unwrap();
    assert!(display.writes > writes);
    assert_eq!(mask.show(), Some(&show));
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert_eq!(mask.scene().unwrap().focused(), focus);
    assert_eq!(draft.face(&basis).unwrap(), face);
    assert!(
        mask.compositor
            .compose_frame(&mut display)
            .unwrap()
            .cursor_visible
    );
    let writes = display.writes;
    mask.pointer_motion(100, 100, &mut display).unwrap();
    assert_eq!(display.writes, writes, "unchanged cursor has no new damage");
}

#[test]
fn cursor_requires_acknowledged_show_and_scanout_loss_retires_it() {
    let (mut mask, draft) = fixture();
    let mut display = CountingDisplay {
        writes: 0,
        fail: false,
    };
    assert!(matches!(
        mask.pointer_motion(10, 10, &mut display),
        Err(NativeFaceMaskError::Face)
    ));
    assert_eq!(display.writes, 0);
    let face = draft
        .face(&mask.birth_basis("arrival/motion-loss"))
        .unwrap();
    mask.present(face, 1, 1, &mut display).unwrap();
    display.fail = true;
    assert!(matches!(
        mask.pointer_motion(10, 10, &mut display),
        Err(NativeFaceMaskError::Compositor(_))
    ));
    assert!(mask.show().is_none());
}
