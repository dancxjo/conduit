use super::protocol::{LineFrame, Output};
use super::session::{Role, Session};

const SOURCE: &str = include_str!("../../../../../../plots/button-across-room/main.conduit");

fn pair() -> ((Session, Output), (Session, Output)) {
    let interaction = crate::source_interaction::admit_source(SOURCE.as_bytes(), 7).unwrap();
    let plan = super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", SOURCE).unwrap();
    let sink = super::plan::accept(plan.plan.clone(), "browser/b", "boot/b").unwrap();
    (
        Session::prepare(Role::Source, plan, 9, interaction.clone()).unwrap(),
        Session::prepare(Role::Sink, sink, 9, interaction).unwrap(),
    )
}

fn frame(output: Output, phase: &str) -> LineFrame {
    let Output::Line { frame, .. } = output else {
        panic!("expected {phase} frame: {output:?}")
    };
    assert_eq!(frame.phase, phase);
    *frame
}

fn transition(pressed: bool, sequence: u32) -> Vec<u8> {
    conduit_semantic_catalog::button_transition_value(
        "button/primary",
        pressed,
        u64::from(sequence),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn canonical_button_runs_ordered_press_release_in_two_kernel_fragments() {
    let ((mut source, mut output), (mut sink, waiting)) = pair();
    assert!(matches!(waiting, Output::Waiting { .. }));
    let mut checked_id = None;
    for (sequence, pressed) in [(0, true), (1, false)] {
        let Output::Input {
            input,
            plan_projection,
            ..
        } = output
        else {
            panic!("expected input: {output:?}")
        };
        assert_eq!(input.host_id, "browser/a");
        assert_eq!(input.request_sequence, sequence);
        assert_eq!(plan_projection.hosts[0].gears.len(), 1);
        assert_eq!(plan_projection.hosts[1].gears.len(), 2);
        assert_eq!(plan_projection.cord.maximum_in_flight_items, 1);
        if let Some(checked) = &checked_id {
            assert_eq!(checked, &plan_projection.checked_plot_id);
        } else {
            let (_, local) =
                crate::plot_runner::TourSession::prepare("browser/a", "boot/a", SOURCE, 9).unwrap();
            assert!(matches!(
                local,
                crate::plot_runner::protocol::TourHostEffect::ButtonTransition(_)
            ));
            let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
            let syntax = conduit_plot::parse_syntax_document(SOURCE);
            let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
            let expanded =
                conduit_plot::expand_canonical_plot(&checked, "button_across_room", &catalog)
                    .unwrap();
            assert_eq!(
                plan_projection.checked_plot_id,
                expanded.checked_plot_id.as_str()
            );
            checked_id = Some(plan_projection.checked_plot_id);
        }
        let value = frame(
            source
                .complete_input(
                    &input.active_play_id,
                    sequence,
                    &transition(pressed, sequence),
                )
                .unwrap(),
            "value",
        );
        assert_eq!(value.sequence, u64::from(sequence));
        assert_eq!(value.payload, transition(pressed, sequence));
        let Output::Manifestation {
            manifestation,
            accepted_frame,
            ..
        } = sink.ingest(value).unwrap()
        else {
            panic!("expected indicator")
        };
        assert_eq!(manifestation.host_id, "browser/b");
        assert_eq!(
            manifestation.text.as_deref(),
            Some(if pressed { "true" } else { "false" })
        );
        assert_eq!(Some(&manifestation.checked_plot_id), checked_id.as_ref());
        assert!(matches!(
            source.ingest(*accepted_frame).unwrap(),
            Output::Waiting { .. }
        ));
        let delivered = frame(sink.complete_manifestation().unwrap(), "delivered");
        output = source.ingest(delivered).unwrap();
    }
    let Output::Input { input, .. } = output else {
        panic!("standing source must request another transition")
    };
    assert_eq!(input.request_sequence, 2);
    let Output::Receipt { receipt, .. } = source.cancel().unwrap() else {
        panic!("expected cancelled source receipt")
    };
    assert_eq!(receipt.transferred_values, 2);
    assert_eq!(receipt.disposition, "cancelled");
    let Output::Receipt { receipt, .. } = sink.cancel().unwrap() else {
        panic!("expected cancelled sink receipt")
    };
    assert_eq!(receipt.transferred_values, 2);
    assert_eq!(receipt.disposition, "cancelled");
}

#[test]
fn stale_input_and_line_sequences_refuse_without_consuming_the_current_request() {
    let ((mut source, output), (mut sink, _)) = pair();
    let Output::Input { input, .. } = output else {
        panic!("expected input")
    };
    let play = input.active_play_id;
    assert!(source
        .complete_input("stale-play", 0, &transition(true, 0))
        .is_err());
    assert!(source
        .complete_input(&play, 1, &transition(true, 0))
        .is_err());
    assert!(source
        .complete_input(&play, 0, b"not typed button Info")
        .is_err());
    let value = frame(
        source
            .complete_input(&play, 0, &transition(true, 0))
            .unwrap(),
        "value",
    );
    let mut wrong = value.clone();
    wrong.sequence = 1;
    assert!(sink.ingest(wrong).is_err());
    let Output::Manifestation { accepted_frame, .. } = sink.ingest(value.clone()).unwrap() else {
        panic!("expected indicator")
    };
    assert!(sink.ingest(value).is_err());
    let mut wrong = *accepted_frame.clone();
    wrong.sequence = 1;
    assert!(source.ingest(wrong).is_err());
    source.ingest(*accepted_frame).unwrap();
    let delivered = frame(sink.complete_manifestation().unwrap(), "delivered");
    assert!(matches!(
        source.ingest(delivered.clone()).unwrap(),
        Output::Input { .. }
    ));
    assert!(source.ingest(delivered).is_err());
    let Output::Receipt { receipt, .. } = source.cancel().unwrap() else {
        panic!("expected cancellation")
    };
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.transferred_values, 1);
    assert!(source
        .complete_input(&play, 1, &transition(false, 1))
        .is_err());
}
