use std::cell::Cell;

use conduit_core::{
    AuthorityTime, EffectAttemptPhase, EffectAttemptState, EffectCommitProfile,
    EffectDiscontinuity, EffectIdempotency, ForeignRetention, Id, InstancePath, PinnedDescriptor,
    PlanResourceBudget, ResourceLeaseContract, ResourceLeasePhase, ResourceLeaseReason,
    ResourceLeaseState, ResourceSharingMode, SemanticHash, UnknownCommitPolicy,
};
use conduit_runtime::{
    DeterministicEffectBackend, DeterministicEffectFault, HostedEffectDisposition, HostedLeaseUse,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn lease_contract() -> ResourceLeaseContract<'static> {
    ResourceLeaseContract {
        schema_version: 1,
        id: Id("lease/hosted-output"),
        resource_binding: Id("resource/hosted-output"),
        holder: InstancePath::new("writer").unwrap(),
        run: Id("run/hosted-1"),
        epoch: 3,
        scope: Id("scope/write"),
        sharing: ResourceSharingMode::Exclusive,
        reservation: PlanResourceBudget {
            memory_bytes: 1024,
            evidence_bytes: 1024,
            ..PlanResourceBudget::ZERO
        },
        time_basis: Id("clock/monotonic"),
        issued_at_tick: 0,
        expires_at_tick: 100,
        revocation_grace_ticks: 5,
        cleanup_ticks: 10,
        maximum_operations: 4,
        maximum_evidence_events: 16,
        cleanup_escalation: pin("cleanup/force-close", 1),
        foreign_retention: ForeignRetention::Unsupported,
    }
}

fn commit_profile() -> EffectCommitProfile<'static> {
    EffectCommitProfile {
        schema_version: 1,
        id: Id("effect/hosted-output"),
        operation: Id("hosted/write"),
        resource_lease: Id("lease/hosted-output"),
        commit_boundary: pin("commit/provider-call", 2),
        idempotency: EffectIdempotency::SameKeySameEffect,
        unknown_commit: UnknownCommitPolicy::RetrySameIdempotencyKey,
        discontinuity: EffectDiscontinuity::CommitUnknown,
        cleanup: pin("cleanup/provider", 3),
        maximum_attempts: 2,
        evidence_events_per_attempt: 4,
    }
}

fn lease_use(tick: u64) -> HostedLeaseUse<'static> {
    HostedLeaseUse {
        resource_binding: Id("resource/hosted-output"),
        holder: InstancePath::new("writer").unwrap(),
        run: Id("run/hosted-1"),
        epoch: 3,
        now: AuthorityTime {
            basis: Id("clock/monotonic"),
            tick,
        },
    }
}

fn attempt(key: &'static str) -> EffectAttemptState<'static> {
    EffectAttemptState::new(commit_profile(), lease_contract(), 1, Some(Id(key))).unwrap()
}

#[test]
fn deterministic_faults_bracket_the_real_commit_call() {
    let committed = Cell::new(false);
    let mut lease = ResourceLeaseState::new(lease_contract()).unwrap();
    let mut successful = attempt("idempotency/success");
    let outcome = DeterministicEffectBackend::new(DeterministicEffectFault::None)
        .execute(&mut lease, &mut successful, lease_use(10), || {
            committed.set(true);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(outcome, HostedEffectDisposition::Acknowledged);
    assert!(committed.get());
    assert!(successful.may_report_success());

    committed.set(false);
    let mut before = attempt("idempotency/before");
    let outcome = DeterministicEffectBackend::new(DeterministicEffectFault::BeforeCommit)
        .execute(&mut lease, &mut before, lease_use(11), || {
            committed.set(true);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(outcome, HostedEffectDisposition::FailedBeforeCommit);
    assert!(!committed.get());
    assert_eq!(before.phase(), EffectAttemptPhase::Failed);

    let mut unknown = attempt("idempotency/unknown");
    let outcome =
        DeterministicEffectBackend::new(DeterministicEffectFault::AfterCommitBeforeAcknowledgement)
            .execute(&mut lease, &mut unknown, lease_use(12), || Ok::<_, ()>(()))
            .unwrap();
    assert_eq!(outcome, HostedEffectDisposition::CommitUnknown);
    assert_eq!(unknown.phase(), EffectAttemptPhase::CommitUnknown);
    assert!(!unknown.may_report_success());
}

#[test]
fn cleanup_fault_remains_pending_until_finite_escalation_deadline() {
    let mut lease = ResourceLeaseState::new(lease_contract()).unwrap();
    let mut effect = attempt("idempotency/cleanup");
    effect.start().unwrap();
    assert!(effect.cancel().is_ok());

    let outcome = DeterministicEffectBackend::new(DeterministicEffectFault::DuringCleanup)
        .cleanup(&mut lease, &mut effect, lease_use(20).now, 1, || {
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(outcome, HostedEffectDisposition::CleanupPending);
    assert_eq!(lease.phase(), ResourceLeasePhase::Cleaning);
    assert_eq!(
        lease.enforce_cleanup_deadline(lease_use(30).now),
        Err(ResourceLeaseReason::CleanupTimeout)
    );
    assert_eq!(lease.phase(), ResourceLeasePhase::Failed);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_file_process_and_socket_witnesses_use_explicit_handles() {
    use std::fs::{File, read};
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use conduit_runtime::{commit_file, commit_process, commit_socket, force_kill_and_wait};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-resource-effect-{}-{nonce}",
        std::process::id()
    ));
    let mut file = File::create(&path).unwrap();
    commit_file(&mut file, b"committed").unwrap();
    drop(file);
    assert_eq!(read(&path).unwrap(), b"committed");
    std::fs::remove_file(&path).unwrap();

    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    let mut child = commit_process(&mut command).unwrap();
    force_kill_and_wait(&mut child).unwrap();
    assert!(child.try_wait().unwrap().is_some());

    let (mut sender, mut receiver) = UnixStream::pair().unwrap();
    commit_socket(&mut sender, b"accepted").unwrap();
    let mut received = [0_u8; 8];
    receiver.read_exact(&mut received).unwrap();
    assert_eq!(&received, b"accepted");
}
