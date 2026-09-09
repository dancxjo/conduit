use super::tests::*;
use super::*;
use crate::tour_workspace::lesson;

#[test]
fn lesson_scroll_changes_prose_pixels_and_preserves_the_laboratory() {
    let (tour, mut shell, mut display) = fixture();
    shell.present(&tour, &mut display).unwrap();
    shell.route_pointer(20, 100, true).unwrap();
    shell.compositor.compose_frame(&mut display).unwrap();
    let before = display.pixels.clone();
    let base = *shell.lesson_scene.as_ref().unwrap();
    let prose = base
        .commands()
        .iter()
        .map(|command| command.payload())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(prose.contains("Conduit lets you make"));
    assert!(prose.contains("nearby Forms are unaffected."));
    shell.route_pointer(20, 100, true).unwrap();
    let ScrollOutcome::Updated(receipt) = shell
        .scroll_focused(ScrollDirection::End, &mut display)
        .unwrap()
    else {
        panic!("lesson must scroll to its final paragraph");
    };
    let visible = lesson::scrolled(&base, receipt.current_offset).unwrap();
    let last = visible.commands().last().unwrap();
    assert!(last.payload().ends_with("nearby Forms are unaffected."));
    assert_eq!(
        last.clip_class(),
        conduit_presentation::GraphicsClipClass::FullyVisible
    );
    let narrative_width = 640 * 46 / 100;
    assert!(
        (40..416)
            .any(|y| (8..narrative_width)
                .any(|x| before[y * 640 + x] != display.pixels[y * 640 + x]))
    );
    for y in 0..40 {
        for x in 8..narrative_width {
            assert_eq!(
                before[y * 640 + x],
                display.pixels[y * 640 + x],
                "lesson header changed while scrolling"
            );
        }
    }
    for y in 0..416 {
        for x in narrative_width..640 {
            assert_eq!(
                before[y * 640 + x],
                display.pixels[y * 640 + x],
                "laboratory pixel changed at {x},{y}"
            );
        }
    }
    assert!(matches!(
        shell
            .scroll_focused(ScrollDirection::End, &mut display)
            .unwrap(),
        ScrollOutcome::Boundary
    ));
    shell.present(&tour, &mut display).unwrap();
    let state = shell
        .surfaces
        .iter()
        .find(|state| state.slot == Slot::Workspace)
        .unwrap();
    assert_eq!(state.scroll.offset(), receipt.current_offset);
    shell
        .scroll_focused(ScrollDirection::Start, &mut display)
        .unwrap();
    // Ignore the pointer cursor: underlying retained pane text must return.
    assert!(
        shell
            .surfaces
            .iter()
            .find(|state| state.slot == Slot::Workspace)
            .unwrap()
            .scroll
            .offset()
            == 0
    );
}
