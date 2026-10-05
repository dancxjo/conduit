use super::common::*;
use conduit_core::claims::*;
#[derive(PartialEq, Eq)]
struct Failure {
    host: u64,
    boot: u64,
    probe: u64,
}
#[derive(PartialEq, Eq)]
enum Diagnosis {
    HostUnavailable,
    PathUnavailable,
}
struct Runtime;
impl ClaimDomain for Runtime {
    type Target = Failure;
    type Value = Diagnosis;
    fn contract(&self) -> ClaimText<'_> {
        text("runtime/diagnosis@1")
    }
    fn validate(&self, target: &Failure, _: &Diagnosis) -> bool {
        target.boot > 0
    }
}

#[test]
fn one_failure_observation_cannot_distinguish_host_from_path() {
    let target = Failure {
        host: 1,
        boot: 2,
        probe: 3,
    };
    let sign = conduit_core::SignIdentity {
        sign_id: "timeout/3".into(),
        host_id: "host/1".into(),
        boot_id: "boot/2".into(),
        active_play_id: None,
        sequence: 3,
    };
    let support = [ClaimSupport::Sign(&sign)];
    let domain = Runtime;
    let host = SemanticClaim::new(
        &domain,
        &target,
        &Diagnosis::HostUnavailable,
        basis("a/host", &support, &[]),
    )
    .unwrap();
    let path = SemanticClaim::new(
        &domain,
        &target,
        &Diagnosis::PathUnavailable,
        basis("b/path", &support, &[]),
    )
    .unwrap();
    let candidates = [&host, &path];
    let resolution = resolve_claims(text("r1"), &candidates, &Conservative).unwrap();
    assert!(matches!(
        resolution.decision(),
        ClaimDecision::Abstained { .. }
    ));
    assert!(resolution.receipts()[..2]
        .iter()
        .all(|r| r.unwrap().exclusion.is_none()));
    assert_eq!(
        resolution.candidates()[0].basis().support,
        resolution.candidates()[1].basis().support
    );
}
