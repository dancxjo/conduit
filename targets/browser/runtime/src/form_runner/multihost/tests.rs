use super::protocol::Output;
use super::session::{Role, Session};

const SOURCE: &str = r#"form hello-across {
    message: text/literal("hello across one planned Cord")
    show: presentation/text
    message > show
}"#;

fn prepare_pair() -> ((Session, Output), (Session, Output)) {
    let interaction = crate::source_interaction::admit_source(SOURCE.as_bytes(), 7).unwrap();
    let source_plan =
        super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", SOURCE).unwrap();
    let sink_plan = super::plan::accept(source_plan.plan.clone(), "browser/b", "boot/b").unwrap();
    (
        Session::prepare(Role::Source, source_plan, 9, interaction.clone()).unwrap(),
        Session::prepare(Role::Sink, sink_plan, 9, interaction).unwrap(),
    )
}

#[test]
fn unchanged_form_executes_two_exact_fragments_over_one_planned_line() {
    let ((mut source, source_output), (mut sink, sink_output)) = prepare_pair();
    assert!(matches!(sink_output, Output::Waiting { .. }));
    let (value, source_projection) = match source_output {
        Output::Line {
            frame,
            plan_projection: Some(projection),
            receipt: None,
            ..
        } => (frame, projection),
        _ => panic!("source did not offer its exact Line value"),
    };
    assert_eq!(value.phase, "value");
    assert_eq!(value.sequence, 0);
    assert_eq!(value.payload, b"hello across one planned Cord");
    assert_eq!(source_projection.hosts.len(), 2);
    assert!(source_projection.cord.crosses_host);
    assert_eq!(source_projection.cord.maximum_in_flight_items, 1);

    let (accepted, sink_projection, manifestation) = match sink.ingest(*value).unwrap() {
        Output::Manifestation {
            accepted_frame,
            plan_projection,
            manifestation,
            ..
        } => (accepted_frame, plan_projection, manifestation),
        _ => panic!("sink did not request its planned presentation"),
    };
    assert_eq!(source_projection.plan_id, sink_projection.plan_id);
    assert_eq!(
        manifestation.text.as_deref(),
        Some("hello across one planned Cord")
    );
    assert_eq!(manifestation.host_id, "browser/b");
    assert!(matches!(
        source.ingest(*accepted).unwrap(),
        Output::Waiting { .. }
    ));

    let delivered = match sink.complete_manifestation().unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("sink did not acknowledge exact delivery"),
    };
    let close = match source.ingest(*delivered).unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("source did not close its exact remote Cord"),
    };
    let (terminal, sink_receipt) = match sink.ingest(*close).unwrap() {
        Output::Line {
            frame,
            receipt: Some(receipt),
            ..
        } => (frame, receipt),
        _ => panic!("sink did not reach terminal truth"),
    };
    assert_eq!(sink_receipt.disposition, "completed");
    assert_eq!(sink_receipt.transferred_values, 1);
    match source.ingest(*terminal).unwrap() {
        Output::Receipt { receipt, .. } => {
            assert_eq!(receipt.disposition, "completed");
            assert_eq!(receipt.transferred_values, 1);
            assert_ne!(receipt.fragment_id, sink_receipt.fragment_id);
            assert_ne!(receipt.active_play_id, sink_receipt.active_play_id);
        }
        _ => panic!("source did not retain terminal truth"),
    }
}

#[test]
fn exact_plan_admission_refuses_a_stale_sink_boot_before_play() {
    let exact = super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", SOURCE).unwrap();
    let error = super::plan::accept(exact.plan, "browser/b", "boot/stale")
        .err()
        .expect("the exact Plan must not move to a stale Boot");
    assert_eq!(
        error,
        "received multi-Host Plan does not name this exact sink Host and Boot"
    );
}

#[test]
fn wrong_boot_frame_refuses_before_remote_admission_and_cancel_is_distinct() {
    let ((mut source, source_output), (mut sink, _)) = prepare_pair();
    let mut value = match source_output {
        Output::Line { frame, .. } => frame,
        _ => panic!("source did not offer a value"),
    };
    value.sink_boot_id.push_str("/stale");
    assert_eq!(
        sink.ingest(*value).unwrap_err(),
        "multi-Host Line frame does not match the exact planned identity"
    );
    match source.cancel().unwrap() {
        Output::Receipt { receipt, .. } => {
            assert_eq!(receipt.disposition, "cancelled");
            assert_eq!(receipt.transferred_values, 0);
        }
        _ => panic!("cancellation did not retain a distinct receipt"),
    }
}

#[test]
fn oversized_line_value_refuses_before_remote_admission() {
    let ((_, source_output), (mut sink, _)) = prepare_pair();
    let mut value = match source_output {
        Output::Line { frame, .. } => frame,
        _ => panic!("source did not offer a value"),
    };
    value.payload = vec![0; crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES + 1];
    assert!(sink.ingest(*value).is_err());
}

#[test]
fn ordinary_transform_can_run_after_the_remote_cord() {
    let source = r#"form too-large {
        message: text/literal("hello")
        upper: text/upper
        show: presentation/text
        message > upper > show
    }"#;
    let plan = super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", source).unwrap();
    assert_eq!(plan.plan.fragments.len(), 2);
    assert_eq!(
        plan.plan
            .fragments
            .iter()
            .map(|fragment| fragment.placements.len())
            .sum::<usize>(),
        3
    );
}
