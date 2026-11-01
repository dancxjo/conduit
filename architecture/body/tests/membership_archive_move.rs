//! Canonical move-seal parity, rollback and allocation custody. CPU evidence;
//! this does not establish ESP32 stack/heap or durable flash acceptance.
use conduit_body::*;
use conduit_core::{BootId, CheckedPlotId, HostId, OfferGeneration, SignId, SourceDocumentId};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn id(n: u64) -> String {
    format!("{n:064x}")
}
fn history(boots: u64) -> BodyBiographyEvidence {
    let body = Body::born(
        SourceDocumentId::from(id(1)),
        CheckedPlotId::from(id(2)),
        1,
        SignId::from(id(3)),
    )
    .unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Thermometer".into(),
    )
    .unwrap();
    let mut membership = evidence.membership.clone();
    let part = PartId::bind(&body.body_id, "local-thermometer", 1).unwrap();
    let proof = MembershipProofId::bind("private-custody-proof").unwrap();
    let mut changes = Vec::new();
    let mut sequence = 2;
    let change = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from(id(1000 + sequence)),
        )
        .unwrap();
    changes.push((change, sequence));
    sequence += 1;
    for boot in 0..boots {
        let boot_id = BootId::from(id(100 + boot));
        let change = membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: HostId::from(id(99)),
                    boot_id: boot_id.clone(),
                    offer_generation: OfferGeneration(1),
                    proof_id: proof.clone(),
                    sequence: boot + 1,
                },
                SignId::from(id(1000 + sequence)),
            )
            .unwrap();
        changes.push((change, sequence));
        sequence += 1;
        if boot + 1 < boots {
            let change = membership
                .observe_offline(
                    &body.body_id,
                    membership.revision,
                    &part,
                    &boot_id,
                    SignId::from(id(1000 + sequence)),
                )
                .unwrap();
            changes.push((change, sequence));
            sequence += 1;
        }
    }
    evidence
        .append_membership_events(membership, &changes)
        .unwrap();
    evidence
}
#[derive(Serialize)]
struct Payload<'a> {
    schema: &'a str,
    body_id: &'a BodyId,
    ordinal: u64,
    previous_digest: Option<[u8; 32]>,
    records: &'a [BodyBiographyRecord],
    wakes: &'a [Wake],
    body_events: &'a [BodyLifecycleEvent],
    membership_events: &'a [MembershipEvent],
}
fn expected(
    evidence: &BodyBiographyEvidence,
) -> (BodyBiographyEvidence, BodyBiographyArchiveSegment) {
    // Frozen prior semantics: clone only in this CPU oracle, outside measurement.
    let records = evidence
        .records
        .iter()
        .filter(|r| {
            matches!(
                r.kind,
                BodyBiographyRecordKind::PartAdmitted { .. }
                    | BodyBiographyRecordKind::HostJoined { .. }
                    | BodyBiographyRecordKind::HostLeft { .. }
                    | BodyBiographyRecordKind::PartRevoked { .. }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let previous_digest = evidence
        .compaction
        .as_ref()
        .and_then(|s| s.archive_head_digest);
    let ordinal = evidence
        .compaction
        .as_ref()
        .map_or(1, |s| s.sealed_segments.saturating_add(1));
    let payload = Payload {
        schema: "conduit.body/biography-archive-segment@2",
        body_id: &evidence.body_id,
        ordinal,
        previous_digest,
        records: &records,
        wakes: &[],
        body_events: &[],
        membership_events: &evidence.membership.events,
    };
    let digest: [u8; 32] = Sha256::digest(postcard::to_allocvec(&payload).unwrap()).into();
    let segment = BodyBiographyArchiveSegment {
        schema: payload.schema.into(),
        body_id: evidence.body_id.clone(),
        ordinal,
        previous_digest,
        records,
        wakes: vec![],
        body_events: vec![],
        membership_events: evidence.membership.events.clone(),
        digest,
    };
    let mut active = evidence.clone();
    active.membership.checkpoint_events().unwrap();
    active
        .records
        .retain(|r| !segment.records.iter().any(|old| old.sign_id == r.sign_id));
    let last = segment.records.last().unwrap();
    let prior = evidence.compaction.as_ref();
    let (through_sequence, through_sign_id) =
        if let Some(p) = prior.filter(|p| p.through_sequence >= last.sequence) {
            (p.through_sequence, p.through_sign_id.clone())
        } else {
            (last.sequence, last.sign_id.clone())
        };
    active.compaction = Some(BodyBiographyCompaction {
        wakes: prior.map_or(0, |p| p.wakes),
        records: prior.map_or(0, |p| p.records) + segment.records.len() as u64,
        through_sequence,
        through_sign_id,
        first_wake_id: prior.and_then(|p| p.first_wake_id.clone()),
        sealed_segments: ordinal,
        archive_head_digest: Some(digest),
    });
    active.validate().unwrap();
    segment.validate_as_head_of(&active).unwrap();
    (active, segment)
}
#[test]
fn real_seventeen_boot_suffix_moves_exact_ids_and_shrinks_active_backing() {
    let mut evidence = history(17);
    assert_eq!(evidence.records.len(), 35);
    let original_record_backing = evidence.records.as_ptr();
    let original_event_backing = evidence.membership.events.as_ptr();
    let historical_signs = evidence
        .records
        .iter()
        .skip(1)
        .map(|record| record.sign_id.as_str().as_ptr())
        .collect::<Vec<_>>();
    let historical_changes = evidence
        .membership
        .events
        .iter()
        .map(|event| event.change_id.as_str().as_ptr())
        .collect::<Vec<_>>();
    let (active, expected_archive) = expected(&evidence);
    let (sealed, memory) =
        allocation_probe::observe(|| evidence.seal_membership_history().unwrap().unwrap());
    assert_eq!(evidence, active);
    assert_eq!(sealed, expected_archive);
    assert_eq!(sealed.records.as_ptr(), original_record_backing);
    assert_eq!(sealed.membership_events.as_ptr(), original_event_backing);
    assert_eq!(
        sealed
            .records
            .iter()
            .map(|record| record.sign_id.as_str().as_ptr())
            .collect::<Vec<_>>(),
        historical_signs
    );
    assert_eq!(
        sealed
            .membership_events
            .iter()
            .map(|event| event.change_id.as_str().as_ptr())
            .collect::<Vec<_>>(),
        historical_changes
    );
    assert_eq!(evidence.records.len(), 1);
    assert_eq!(evidence.records.capacity(), 1);
    assert_eq!(
        postcard::to_allocvec(&sealed).unwrap(),
        postcard::to_allocvec(&expected_archive).unwrap()
    );
    println!(
        "17Boot membership move seal CPU additional peak={} requested={} archive={} active={}",
        memory.peak_bytes,
        memory.requested_bytes,
        postcard::to_allocvec(&sealed).unwrap().len(),
        postcard::to_allocvec(&evidence).unwrap().len()
    );
    let mut short = history(1);
    let (_, short_memory) =
        allocation_probe::observe(|| short.seal_membership_history().unwrap().unwrap());
    assert!(memory.peak_bytes <= short_memory.peak_bytes + 512,
        "historical growth must move existing backing, not allocate copied IDs: short={short_memory:?} long={memory:?}");
    assert!(
        memory.peak_bytes < 8192,
        "must not clone biography/history or buffer archive payload: {memory:?}"
    );
}
fn append_offline(evidence: &mut BodyBiographyEvidence) {
    let mut membership = evidence.membership.clone();
    let part = membership.parts[0].part_id.clone();
    let boot = membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .boot_id
        .clone();
    let sequence = evidence
        .records
        .iter()
        .map(|r| r.sequence)
        .chain(evidence.compaction.iter().map(|s| s.through_sequence))
        .max()
        .unwrap()
        + 1;
    let change = membership
        .observe_offline(
            &evidence.body_id,
            membership.revision,
            &part,
            &boot,
            SignId::from(id(1000 + sequence)),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(change, sequence)])
        .unwrap();
}
#[test]
fn cumulative_chain_and_existing_checkpoint_match_prior_canonical_semantics() {
    let mut evidence = history(2);
    let first = evidence.seal_membership_history().unwrap().unwrap();
    append_offline(&mut evidence);
    let (active, archive) = expected(&evidence);
    let second = evidence.seal_membership_history().unwrap().unwrap();
    assert_eq!(second, archive);
    assert_eq!(evidence, active);
    assert_eq!(second.previous_digest, Some(first.digest));
    assert_eq!(second.ordinal, 2);
    assert!(evidence.seal_membership_history().unwrap().is_none());
}
#[test]
fn post_move_capacity_refusal_restores_original_records_events_checkpoint_and_summary() {
    let mut evidence = history(2);
    evidence.seal_membership_history().unwrap().unwrap();
    append_offline(&mut evidence);
    evidence.compaction.as_mut().unwrap().records = u64::MAX;
    evidence.validate().unwrap();
    let original = evidence.clone();
    let records = evidence.records.as_ptr();
    let events = evidence.membership.events.as_ptr();
    let record_capacity = evidence.records.capacity();
    let event_capacity = evidence.membership.events.capacity();
    assert_eq!(
        evidence.seal_membership_history(),
        Err(BodyBiographyError::CapacityExhausted)
    );
    assert_eq!(evidence, original);
    assert_eq!(evidence.records.as_ptr(), records);
    assert_eq!(evidence.membership.events.as_ptr(), events);
    assert_eq!(evidence.records.capacity(), record_capacity);
    assert_eq!(evidence.membership.events.capacity(), event_capacity);
    evidence.validate().unwrap();
}

#[test]
fn malformed_preflight_refuses_without_any_custody_change() {
    let mut evidence = history(3);
    evidence.membership.events[0].revision.0 += 1;
    let original = evidence.clone();
    let records = evidence.records.as_ptr();
    let events = evidence.membership.events.as_ptr();
    assert!(evidence.seal_membership_history().is_err());
    assert_eq!(evidence, original);
    assert_eq!(evidence.records.as_ptr(), records);
    assert_eq!(evidence.membership.events.as_ptr(), events);
}

#[test]
fn legacy_archive_validation_keeps_exact_original_postcard_digest() {
    #[derive(Serialize)]
    struct LegacyPayload<'a> {
        schema: &'a str,
        body_id: &'a BodyId,
        ordinal: u64,
        previous_digest: Option<[u8; 32]>,
        records: &'a [BodyBiographyRecord],
        wakes: &'a [Wake],
    }
    let evidence = history(1);
    let sign = SignId::from(id(700));
    let (_, wake) = evidence.body.wake(1, sign.clone()).unwrap();
    let records = vec![BodyBiographyRecord {
        sequence: 100,
        sign_id: sign,
        kind: BodyBiographyRecordKind::WakeEvent {
            wake_id: wake.wake_id.clone(),
            event_index: 0,
        },
    }];
    let wakes = vec![wake];
    let payload = LegacyPayload {
        schema: "conduit.body/biography-archive-segment@1",
        body_id: &evidence.body_id,
        ordinal: 1,
        previous_digest: None,
        records: &records,
        wakes: &wakes,
    };
    let digest: [u8; 32] = Sha256::digest(postcard::to_allocvec(&payload).unwrap()).into();
    let mut archive = BodyBiographyArchiveSegment {
        schema: payload.schema.into(),
        body_id: evidence.body_id,
        ordinal: 1,
        previous_digest: None,
        records,
        wakes,
        body_events: vec![],
        membership_events: vec![],
        digest,
    };
    archive.validate().unwrap();
    archive.digest[0] ^= 1;
    assert!(archive.validate().is_err());
}

#[path = "../../core/tests/common/sealed_state.rs"]
mod core_fixture;
fn proposal(evidence: &BodyBiographyEvidence) -> (BodyPlotPlan, HostId, BootId) {
    let resident = evidence.body.workset.plots()[0].clone();
    let current = evidence.membership.parts[0].current.as_ref().unwrap();
    let mut fragment = core_fixture::fragment();
    fragment.source_document_id = resident.source_document_id.clone();
    fragment.checked_plot_id = resident.checked_plot_id.clone();
    fragment.expanded_plot_id = id(900).into();
    fragment.host_id = current.host_id.clone();
    fragment.boot_id = current.boot_id.clone();
    fragment.offer_generation = current.offer_generation;
    for placement in &mut fragment.placements {
        placement.host_id = current.host_id.clone();
        placement.boot_id = current.boot_id.clone();
    }
    (
        BodyPlotPlan {
            plot: resident,
            plan: core_fixture::seal(fragment),
        },
        current.host_id.clone(),
        current.boot_id.clone(),
    )
}
#[test]
fn public_checkpoint_seal_retains_pending_custody_and_fences_proposed_and_started_play() {
    let evidence = history(3);
    let (partition, host, boot) = proposal(&evidence);
    let mut owner = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    assert!(owner.seal_membership_history_for_checkpoint().unwrap());
    assert_eq!(owner.pending_archives().len(), 1);
    owner.pending_archives()[0]
        .validate_as_head_of(owner.evidence())
        .unwrap();
    let pending = owner.pending_archives().to_vec();
    assert!(!owner.seal_membership_history_for_checkpoint().unwrap());
    assert_eq!(owner.pending_archives(), pending);
    owner.propose(vec![partition], &host, &boot).unwrap();
    let proposed = postcard::to_allocvec(owner.evidence()).unwrap();
    assert_eq!(
        owner.seal_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::NotLulled)
    );
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), proposed);
    let current = owner.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |n| conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), n).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    owner.started(&host, &boot, play, wake).unwrap();
    let started = postcard::to_allocvec(owner.evidence()).unwrap();
    let active = owner.realization().unwrap().play.clone();
    assert_eq!(
        owner.seal_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::NotLulled)
    );
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), started);
    assert_eq!(owner.realization().unwrap().play, active);
    assert_eq!(owner.pending_archives(), pending);
}
#[test]
fn public_checkpoint_seal_counter_failure_keeps_every_owner_field_and_pending_segment() {
    let mut evidence = history(2);
    evidence.seal_membership_history().unwrap().unwrap();
    append_offline(&mut evidence);
    evidence.compaction.as_mut().unwrap().records = u64::MAX;
    evidence.validate().unwrap();
    let mut owner = SealedBodyLifecycleSession::open(evidence).unwrap();
    let before = postcard::to_allocvec(owner.evidence()).unwrap();
    let foreground = owner.foreground().cloned();
    assert_eq!(
        owner.seal_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::Biography(
            BodyBiographyError::CapacityExhausted
        ))
    );
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), before);
    assert_eq!(owner.foreground(), foreground.as_ref());
    assert!(owner.pending_archives().is_empty());
    assert!(owner.realization().is_none());
}

#[test]
fn public_checkpoint_seal_full_pending_bound_preserves_unsealed_current_suffix() {
    let mut evidence = history(1);
    let mut membership = evidence.membership.clone();
    let mut changes = Vec::new();
    let mut remotes = Vec::new();
    let mut sequence = evidence.last_sequence() + 1;
    for index in 0..8 {
        let part = PartId::bind(&evidence.body_id, &format!("remote/{index}"), index + 2).unwrap();
        let proof = MembershipProofId::bind(&format!("proof/{index}")).unwrap();
        let admitted = membership
            .admit(
                &evidence.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                SignId::from(id(5000 + sequence)),
            )
            .unwrap();
        changes.push((admitted, sequence));
        sequence += 1;
        let host = HostId::from(id(2000 + index));
        let boot = BootId::from(id(3000 + index));
        let joined = membership
            .observe_present(
                &evidence.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: host.clone(),
                    boot_id: boot.clone(),
                    offer_generation: OfferGeneration(1),
                    proof_id: proof,
                    sequence: 1,
                },
                SignId::from(id(5000 + sequence)),
            )
            .unwrap();
        changes.push((joined, sequence));
        sequence += 1;
        remotes.push((host, boot));
    }
    evidence
        .append_membership_events(membership, &changes)
        .unwrap();
    let host = evidence.membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .host_id
        .clone();
    let boot = evidence.membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .boot_id
        .clone();
    let mut owner = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    assert!(owner.seal_membership_history_for_checkpoint().unwrap());
    for (remote, remote_boot) in &remotes[..7] {
        owner
            .observe_host_lost(remote, remote_boot, &host, &boot)
            .unwrap();
        assert!(owner.seal_membership_history_for_checkpoint().unwrap());
    }
    assert_eq!(owner.pending_archives().len(), MAX_BODY_BIOGRAPHY_WAKES);
    assert!(!owner.seal_membership_history_for_checkpoint().unwrap());
    let (remote, remote_boot) = &remotes[7];
    owner
        .observe_host_lost(remote, remote_boot, &host, &boot)
        .unwrap();
    let before = postcard::to_allocvec(owner.evidence()).unwrap();
    let pending = owner.pending_archives().to_vec();
    assert_eq!(
        owner.seal_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::ArchivePersistenceRequired)
    );
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), before);
    assert_eq!(owner.pending_archives(), pending);
    assert!(owner.realization().is_none());
}

#[test]
fn started_membership_seal_preserves_actual_realization_and_restart_fence() {
    let evidence = history(3);
    let (partition, host, boot) = proposal(&evidence);
    let mut owner = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    assert_eq!(
        owner.seal_started_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::NoProposal)
    );
    owner.propose(vec![partition], &host, &boot).unwrap();
    let before = postcard::to_allocvec(owner.evidence()).unwrap();
    assert_eq!(
        owner.seal_started_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::StalePlay)
    );
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), before);
    let current = owner.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |n| conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), n).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    owner.started(&host, &boot, play.clone(), wake).unwrap();
    let realization = owner.realization().unwrap().clone();
    let body = owner.evidence().body.clone();
    let wakes = owner.evidence().wakes.clone();
    let foreground = owner.foreground().cloned();
    let membership = owner.evidence().membership.clone();
    let sequence = owner.evidence().last_sequence();
    assert!(owner
        .seal_started_membership_history_for_checkpoint()
        .unwrap());
    assert_eq!(owner.realization(), Some(&realization));
    assert_eq!(owner.evidence().body, body);
    assert_eq!(owner.evidence().wakes, wakes);
    assert_eq!(owner.foreground(), foreground.as_ref());
    assert_eq!(owner.evidence().membership.parts, membership.parts);
    assert_eq!(owner.evidence().membership.revision, membership.revision);
    assert_eq!(owner.evidence().last_sequence(), sequence);
    assert!(owner.evidence().membership.events.is_empty());
    let segment = &owner.pending_archives()[0];
    assert_eq!(segment.membership_events, membership.events);
    segment.validate_as_head_of(owner.evidence()).unwrap();
    owner.evidence().validate().unwrap();
    assert!(matches!(
        SealedBodyLifecycleSession::open(owner.evidence().clone()),
        Err(BodyLifecycleSessionError::UnreconciledWake)
    ));
    let resumed = SealedBodyLifecycleSession::resume_here(
        owner.evidence().clone(),
        &host,
        &BootId::from(id(999)),
    )
    .unwrap();
    assert!(resumed.realization().is_none());
    assert!(matches!(resumed.evidence().body.state, BodyState::Lulled));
    assert_ne!(
        resumed.evidence().wakes.last().unwrap().lifecycle,
        owner.evidence().wakes.last().unwrap().lifecycle
    );
    assert_eq!(owner.realization().unwrap().play.as_ref(), Some(&play));
    assert!(!owner
        .seal_started_membership_history_for_checkpoint()
        .unwrap());
    assert_eq!(owner.pending_archives().len(), 1);
}

#[test]
fn started_membership_seal_failure_keeps_exact_owner_and_archive_custody() {
    let mut evidence = history(2);
    evidence.seal_membership_history().unwrap().unwrap();
    append_offline(&mut evidence);
    evidence.compaction.as_mut().unwrap().records = u64::MAX;
    evidence.validate().unwrap();
    // Reattach the local Host so this is an ordinary actual admitted owner.
    let current = evidence.membership.parts[0].clone();
    let mut membership = evidence.membership.clone();
    let host = HostId::from(id(99));
    let boot = BootId::from(id(1001));
    let sequence = evidence.last_sequence() + 1;
    let change = membership
        .observe_present(
            &evidence.body_id,
            membership.revision,
            &current.part_id,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: MembershipProofId::bind("private-custody-proof").unwrap(),
                sequence: 100,
            },
            SignId::from(id(2000)),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(change, sequence)])
        .unwrap();
    let (partition, host, boot) = proposal(&evidence);
    let mut owner = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    owner.propose(vec![partition], &host, &boot).unwrap();
    let current = owner.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |n| conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), n).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    owner.started(&host, &boot, play, wake).unwrap();
    let before = postcard::to_allocvec(owner.evidence()).unwrap();
    let realization = owner.realization().cloned();
    let foreground = owner.foreground().cloned();
    assert_eq!(
        owner.seal_started_membership_history_for_checkpoint(),
        Err(BodyLifecycleSessionError::Biography(
            BodyBiographyError::CapacityExhausted
        ))
    );
    assert_eq!(postcard::to_allocvec(owner.evidence()).unwrap(), before);
    assert_eq!(owner.realization(), realization.as_ref());
    assert_eq!(owner.foreground(), foreground.as_ref());
    assert!(owner.pending_archives().is_empty());
}

#[test]
fn membership_append_refusal_restores_exact_records_membership_capacity_and_backing() {
    for archived in [false, true] {
        let mut evidence = history(3);
        if archived {
            evidence.seal_membership_history().unwrap().unwrap();
        }
        let mut next = evidence.clone();
        append_offline(&mut next);
        let change = next.membership.events.last().unwrap().change_id.clone();
        let sequence = next.last_sequence();
        evidence.records.reserve_exact(17);
        evidence.membership.events.reserve_exact(9);
        let before = postcard::to_allocvec(&evidence).unwrap();
        let pointers = (
            evidence.records.as_ptr(),
            evidence.membership.events.as_ptr(),
            evidence.membership.parts.as_ptr(),
        );
        let capacities = (
            evidence.records.capacity(),
            evidence.membership.events.capacity(),
            evidence.membership.parts.capacity(),
        );
        // The first record has been installed when the second duplicate refuses.
        assert_eq!(
            evidence.append_membership_events(
                next.membership.clone(),
                &[(change.clone(), sequence), (change.clone(), sequence + 1)]
            ),
            Err(BodyBiographyError::DuplicateEvidence)
        );
        assert_eq!(postcard::to_allocvec(&evidence).unwrap(), before);
        assert_eq!(
            (
                evidence.records.as_ptr(),
                evidence.membership.events.as_ptr(),
                evidence.membership.parts.as_ptr()
            ),
            pointers
        );
        assert_eq!(
            (
                evidence.records.capacity(),
                evidence.membership.events.capacity(),
                evidence.membership.parts.capacity()
            ),
            capacities
        );
        // Full validation can also refuse after all selected records are staged.
        let mut malformed = next.membership;
        malformed.revision.0 += 1;
        assert!(evidence
            .append_membership_events(malformed, &[(change, sequence)])
            .is_err());
        assert_eq!(postcard::to_allocvec(&evidence).unwrap(), before);
        assert_eq!(
            (
                evidence.records.as_ptr(),
                evidence.membership.events.as_ptr(),
                evidence.membership.parts.as_ptr()
            ),
            pointers
        );
        assert_eq!(
            (
                evidence.records.capacity(),
                evidence.membership.events.capacity(),
                evidence.membership.parts.capacity()
            ),
            capacities
        );
        evidence.validate().unwrap();
    }
}
