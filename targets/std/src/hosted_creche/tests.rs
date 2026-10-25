//! Real checked/sealed Plan lifecycle and retained-evidence software proof.
use super::test_support::Adapter;
use super::*;
use conduit_body_make::deployment_runtime::CrecheDisposition;
fn planned(adapter: &Adapter) -> (HostAdvertisement, BodyPlan, BodyPlayIdentity) {
    let (host, plan, play, _) = super::test_support::planned(adapter);
    (host, plan, play)
}

fn poll_after_wait(owner: &mut CrecheHost<'_>) -> Result<CrechePoll, String> {
    std::thread::sleep(owner.poll_wait());
    owner.poll()
}

#[test]
fn prospective_preparation_cannot_begin_and_effect_runs_once_after_confirmation() {
    let mut adapter = Adapter::new();
    let (host, plan, play) = planned(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    let request = owner.request();
    assert!(owner.begin(request).is_err());
    assert!(owner
        .confirm_started(&host, &BodyPlayIdentity::bind(&plan, 1))
        .is_err());
    owner.confirm_started(&host, &play).unwrap();
    assert!(owner.begin(CrecheRequest([0; 32])).is_err());
    assert_eq!(owner.begin(request), Ok(CrechePoll::Pending));
    assert!(owner.begin(request).is_err());
    let first = poll_after_wait(&mut owner).unwrap();
    assert!(matches!(first, CrechePoll::Outcome(_)));
    assert_eq!(owner.poll().unwrap(), first); // retained output pressure never repeats an effect
    owner.retire().unwrap();
    owner.retire().unwrap();
    drop(owner);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (1, 1, 1));
}

#[test]
fn execution_identity_is_bound_even_when_reviewed_selection_is_identical() {
    let mut adapter = Adapter::new();
    let (host, plan, play) = planned(&adapter);
    let mut first = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    let request = first.request();
    first.retire().unwrap();
    drop(first);
    let mut next = CrecheHost::prepare(
        &mut adapter,
        &host,
        &plan,
        &BodyPlayIdentity::bind(&plan, 1),
    )
    .unwrap();
    assert_ne!(request, next.request());
    next.retire().unwrap();
}

#[test]
fn invalid_actual_host_and_corrupt_custody_refuse_before_completion() {
    let mut adapter = Adapter::new();
    let (host, plan, play) = planned(&adapter);
    let mut invalid = host.clone();
    invalid
        .resources
        .iter_mut()
        .find(|r| r.pool_id == adapter.resource.pool_id)
        .unwrap()
        .capacity_units = 2;
    assert!(CrecheHost::prepare(&mut adapter, &invalid, &plan, &play).is_err());
    assert_eq!(adapter.drains, 1);
    assert_eq!(adapter.begins, 0);
    adapter.corrupt = true;
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    assert!(poll_after_wait(&mut owner).is_err());
    assert!(poll_after_wait(&mut owner).is_err());
    owner.retire().unwrap();
}

#[test]
fn finite_poll_exhaustion_requires_drain_and_cannot_restart_effects() {
    let mut adapter = Adapter::new();
    adapter.pending = true;
    let (host, plan, play) = planned(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    assert_eq!(poll_after_wait(&mut owner).unwrap(), CrechePoll::Pending);
    assert_eq!(poll_after_wait(&mut owner).unwrap(), CrechePoll::Pending);
    assert!(poll_after_wait(&mut owner).is_err());
    assert!(owner.begin(owner.request()).is_err());
    owner.retire().unwrap();
    drop(owner);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (1, 2, 1));
}

#[test]
fn evidence_size_and_digest_are_separate_custody_requirements() {
    let bytes = b"exact canonical evidence";
    let outcome = CrecheOutcome {
        request: CrecheRequest([1; 32]),
        disposition: CrecheDisposition::Refused,
        evidence_digest: Sha256::digest(bytes).into(),
    };
    let limits = CrecheLimits {
        maximum_evidence_bytes: 1024,
        maximum_polls: 1,
        minimum_poll_interval_ms: 1,
    };
    assert!(verify_evidence(&outcome, bytes, limits).is_ok());
    assert!(verify_evidence(&outcome, b"changed", limits).is_err());
    assert!(verify_evidence(
        &outcome,
        bytes,
        CrecheLimits {
            maximum_evidence_bytes: 1,
            ..limits
        }
    )
    .is_err());
    assert!(!CrecheLimits {
        maximum_evidence_bytes: 64 * 1024 * 1024 + 1,
        ..limits
    }
    .valid());
    for invalid_interval in [0, 1001] {
        assert!(!CrecheLimits {
            minimum_poll_interval_ms: invalid_interval,
            ..limits
        }
        .valid());
    }
}
