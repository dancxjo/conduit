//! Exact lifecycle startup through checked forms and the ordinary browser kernel.
use super::{tests::request_from_sources, *};
use conduit_body::BodyBiographyEvidence;

fn source(kind: &str) -> String {
    format!("form startup {{\n wake: {kind}\n show: presentation/bool-value\n wake.pulse > show.value\n}}\n")
}
fn next(history: &BodyBiographyEvidence) -> u64 {
    history.records.last().unwrap().sequence + 1
}
fn execute(request: BodyStartRequest) -> (BodyBiographyEvidence, usize) {
    let mut history = request.body_evidence.clone().unwrap();
    let (mut session, started) = prepare(request).unwrap();
    history
        .append_wake(
            history.body.clone(),
            started.wake_at_start.clone(),
            next(&history),
        )
        .unwrap();
    let mut progress = started.progress;
    let mut pulses = 0;
    for _ in 0..8 {
        match progress {
            TourProgress::Effect(effect) => {
                let TourHostEffect::Manifestation(effect) = *effect else {
                    panic!("unexpected effect");
                };
                assert_eq!(effect.text.as_deref(), Some("true"));
                pulses += 1;
                progress = session.advance().unwrap();
            }
            TourProgress::Waiting {
                disposition,
                pending_effects,
                ..
            } => {
                assert_eq!(disposition, "quiescent_awaiting_input");
                assert_eq!(pending_effects, 0);
                // Re-observation of this live Play cannot restart the source.
                assert!(matches!(
                    session.poll_effect().unwrap(),
                    TourProgress::Waiting {
                        pending_effects: 0,
                        ..
                    }
                ));
                assert_eq!(session.cancel().unwrap().disposition, "cancelled");
                let wake = started
                    .wake_at_start
                    .lull(format!("sign/lull/{}", started.play.wake_id.as_str()).into())
                    .unwrap();
                let body = history
                    .body
                    .retain_after_lull(
                        &wake,
                        format!("sign/retained/{}", started.play.wake_id.as_str()).into(),
                    )
                    .unwrap();
                history.append_wake(body, wake, next(&history)).unwrap();
                return (history, pulses);
            }
            other => panic!("startup must settle in its living Play: {other:?}"),
        }
    }
    panic!("startup did not settle within its finite pulse budget");
}

#[test]
fn retained_body_first_wake_fires_once_but_ordinary_wake_fires_again() {
    for (kind, expected_later) in [("body/first-wake", 0), ("body/wake", 1)] {
        let source = source(kind);
        let request = request_from_sources(&[&source]);
        let (history, first) = execute(request);
        assert_eq!(first, 1);
        let bytes = serde_json::to_vec(&history).unwrap();
        let mut restored: BodyBiographyEvidence = serde_json::from_slice(&bytes).unwrap();
        let (body, wake) = restored.body.wake(2, "sign/wake-again".into()).unwrap();
        restored
            .append_wake(body, wake.clone(), next(&restored))
            .unwrap();
        let mut later = request_from_sources(&[&source]);
        later.plan = BodyPlan::seal(&wake, later.plan.forms).unwrap();
        later.wake = wake;
        later.play_sequence += 1;
        later.body_evidence = Some(restored);
        let (_, count) = execute(later);
        assert_eq!(count, expected_later);
    }
}

#[test]
fn startup_sources_refuse_missing_or_altered_history_before_execution() {
    let source = source("body/first-wake");
    let mut missing = request_from_sources(&[&source]);
    missing.body_evidence = None;
    assert!(prepare(missing)
        .err()
        .unwrap()
        .contains("no admitted lifecycle evidence"));
    let mut altered = request_from_sources(&[&source]);
    altered.body_evidence.as_mut().unwrap().records.clear();
    assert!(prepare(altered).err().unwrap().contains("startup history"));
    assert!(TourSession::prepare("host/test", "boot/test", &source, 1)
        .err()
        .unwrap()
        .contains("no admitted lifecycle evidence"));
}

#[test]
fn combined_body_lowering_reports_exact_capacity_and_provenance() {
    let sources = (0..8)
        .map(|index| format!(
            "form capacity-{index} {{\n a: text/literal(\"a\")\n show-a: presentation/text\n b: text/literal(\"b\")\n show-b: presentation/text\n c: text/literal(\"c\")\n show-c: presentation/text\n a > show-a\n b > show-b\n c > show-c\n}}\n"
        ))
        .collect::<Vec<_>>();
    let refs = sources.iter().map(String::as_str).collect::<Vec<_>>();
    let request = request_from_sources(&refs);
    let plan_id = request.plan.plan_id.clone();
    let checked = request
        .plan
        .forms
        .iter()
        .map(|form| form.form.checked_form_id.clone())
        .collect::<Vec<_>>();
    let error = match prepare(request) {
        Err(error) => error,
        Ok(_) => panic!("combined Body unexpectedly fit the browser bound"),
    };
    assert!(error.message.contains("Body lowering"));
    let rejection = error
        .rejections
        .first()
        .expect("structured capacity rejection");
    assert_eq!(rejection.reason_code, "lowering.capacity");
    assert_eq!(rejection.category, "Capacity");
    assert!(rejection.required > rejection.available);
    assert_eq!(rejection.plan_id.as_ref(), Some(&plan_id));
    assert_eq!(rejection.checked_form_ids, checked);
    assert_eq!(rejection.host_id.as_str(), "body-host");
    assert_eq!(rejection.boot_id.as_str(), "body-boot");
}
