//! Actual owner/kernel cancellation with refusal-only software presentation.
//! No native rendering, artifact manufacture or successful image is asserted.
use super::*;
use std::time::{Duration, Instant};
struct Other;
impl conduit_std_host::body_execution::BodyForeOutputAdapter for Other {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("unexpected external output".into())
    }
}
#[test]
fn actual_cancelled_adapter_evidence_is_saved_without_claiming_prepared() {
    let (mut owner, root, adapter, controller, _) =
        crate::durable_host::owner::published_creche_fixture(
            include_str!("../../../../../../plots/creche/main.conduit"),
            false,
        );
    let mut worker = owner
        .start_creche_run(&root, 30_000, adapter, Box::new(Other))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < deadline);
        match worker.progress(&mut owner, &root, |_, _| {
            Err("diagnostic has no native scanout".into())
        }) {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => assert_eq!(e, "diagnostic has no native scanout"),
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let completion = worker.take_completion().unwrap();
    let facts = CompletionFacts {
        outcome_acknowledged: completion.outcome_acknowledged(),
        stopped_after_outcome: false,
    };
    let (adapter, delivery, _) = completion.into_parts();
    let held = adapter
        .retained_outcome()
        .expect("actual cancellation evidence retained");
    assert_eq!(held.disposition, CrecheDisposition::Cancelled);
    let manufacture = Custody::default();
    assert!(finish(
        &controller,
        &adapter,
        delivery.as_ref(),
        &owner.truth()["last_execution"],
        &manufacture,
        facts,
        &root,
        Some("diagnostic has no native scanout"),
        "diagnostic-only/not-a-native-image"
    )
    .is_err());
    let evidence = std::fs::read(root.join("canonical-evidence.bin")).unwrap();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&evidence)),
        held.evidence_digest
    );
    assert_eq!(evidence, adapter.retained_evidence(&held).unwrap());
    let record: Value =
        serde_json::from_slice(&std::fs::read(root.join("terminal.json")).unwrap()).unwrap();
    assert_eq!(record["adapter_disposition"], "cancelled");
    assert_eq!(record["prepared"], false);
    assert_eq!(record["boot_observed"], false);
    assert_eq!(record["retention_errors"], json!([]));
    assert!(record["execution"]["play"].is_object());
    std::fs::remove_dir_all(root).unwrap();
}
