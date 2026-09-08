use super::protocol::Output;
use super::session::{Role, Session};

const SOURCE: &str = include_str!("../../../../../../forms/night-radio/main.conduit");

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
fn canonical_night_radio_retains_then_reconstructs_one_report_across_the_planned_line() {
    let prepared =
        super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", SOURCE).unwrap();
    let kinds = prepared
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| fragment.placements.iter())
        .map(|placement| placement.kind_id.as_str())
        .collect::<Vec<_>>();
    for required in [
        conduit_net::TYPED_RECORD_FRAME_KIND,
        conduit_net::ORDERED_RECORD_QUEUE_KIND,
        conduit_net::TYPED_RECORD_DEFRAME_KIND,
    ] {
        assert!(kinds.contains(&required), "Night Radio omitted {required}");
    }

    let ((mut sender, source_output), (mut receiver, sink_output)) = prepare_pair();
    assert!(matches!(sink_output, Output::Waiting { .. }));
    let offered = match source_output {
        Output::Line {
            frame,
            plan_projection: Some(projection),
            ..
        } => {
            assert_eq!(projection.hosts.len(), 2);
            assert_eq!(projection.cord.line_id, "tour/browser-memory-line");
            assert_eq!(projection.cord.maximum_in_flight_items, 1);
            frame
        }
        _ => panic!("Night Radio did not offer its retained report"),
    };
    let accepted = match receiver.ingest(*offered).unwrap() {
        Output::Manifestation {
            accepted_frame,
            manifestation,
            ..
        } => {
            assert_eq!(manifestation.text.as_deref(), Some("NIGHT REPORT"));
            accepted_frame
        }
        _ => panic!("Night Radio did not reconstruct the remote report"),
    };
    assert!(matches!(
        sender.ingest(*accepted).unwrap(),
        Output::Waiting { .. }
    ));
    let delivered = match receiver.complete_manifestation().unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("Night Radio receiver did not acknowledge manifestation"),
    };
    let close = match sender.ingest(*delivered).unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("Night Radio sender did not close the Line"),
    };
    let terminal = match receiver.ingest(*close).unwrap() {
        Output::Line {
            frame,
            receipt: Some(receipt),
            ..
        } => {
            assert_eq!(receipt.disposition, "completed");
            frame
        }
        _ => panic!("Night Radio receiver did not retain terminal truth"),
    };
    let source_receipt = match sender.ingest(*terminal).unwrap() {
        Output::Receipt { receipt, .. } => receipt,
        _ => panic!("Night Radio sender did not complete"),
    };
    assert_eq!(source_receipt.disposition, "completed");
    assert_eq!(source_receipt.transcript.as_ref().unwrap().retention_gap, 0);
}
