//! Public canonical transaction rollback and retained-history allocation proof.
//! Core Plans are verified stand-ins; no native execution or physical claim.
use conduit_body::*;
use conduit_core as c;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../core/tests/common/sealed_state.rs"]
mod core_fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn fixture(boots: u64) -> (SealedBodyLifecycleSession, c::HostId, c::BootId) {
    let host: c::HostId = "esp32/b0a732286178".into();
    let mut boot: c::BootId = format!("boot/{:032x}", 0).into();
    let workset = BodyWorkset::from_plots((1..=2).map(|n| {
        ResidentPlot::new(
            format!("{n:064x}").into(),
            format!("{:064x}", n + 10).into(),
        )
    }))
    .unwrap();
    let sign = |at| c::bind_sign(&host, &boot, None, at).sign_id;
    let body = Body::born_with_plots(workset, 1, sign(1)).unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "x".repeat(21)).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            sign(2),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: c::OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            sign(3),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let mut owner = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    for number in 1..boots {
        boot = format!("boot/{number:032x}").into();
        owner = SealedBodyLifecycleSession::resume_here(owner.evidence().clone(), &host, &boot)
            .unwrap();
        assert!(owner.pending_archives().is_empty());
    }
    (owner, host, boot)
}
fn proposal(
    owner: &SealedBodyLifecycleSession,
    host: &c::HostId,
    boot: &c::BootId,
) -> PreparedVerifiedBodyProposal {
    let verified = owner
        .evidence()
        .body
        .workset
        .plots()
        .iter()
        .map(|plot| {
            let mut fragment = core_fixture::fragment();
            fragment.source_document_id = plot.source_document_id.clone();
            fragment.checked_plot_id = plot.checked_plot_id.clone();
            fragment.expanded_plot_id =
                format!("expanded/{}", plot.checked_plot_id.as_str()).into();
            fragment.host_id = host.clone();
            fragment.boot_id = boot.clone();
            for placement in &mut fragment.placements {
                placement.host_id = host.clone();
                placement.boot_id = boot.clone();
            }
            (
                plot.clone(),
                c::verify_plan_metadata_with_configurations(core_fixture::seal(fragment), |_| {
                    Ok::<_, ()>(vec![])
                })
                .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let inputs = verified
        .iter()
        .map(|(plot, plan)| VerifiedBodyPlotPlan {
            plot: plot.clone(),
            plan,
        })
        .collect::<Vec<_>>();
    owner
        .prepare_verified_proposal(&inputs, &[], None, host, boot)
        .unwrap()
}
fn start(
    owner: &SealedBodyLifecycleSession,
    host: &c::HostId,
    boot: &c::BootId,
) -> (BodyPlayIdentity, Wake) {
    let current = owner.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |at| c::bind_sign(host, boot, Some(&play.active_play_id), at).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    (play, wake)
}
fn history_pointer(evidence: &BodyBiographyEvidence) -> *const u8 {
    evidence.membership.events[0].body_id.as_str().as_ptr()
}
fn assert_unchanged(
    after: &BodyBiographyEvidence,
    before: &BodyBiographyEvidence,
    pointer: *const u8,
) {
    assert_eq!(after, before);
    assert_eq!(
        serde_json::to_vec(after).unwrap(),
        serde_json::to_vec(before).unwrap()
    );
    assert_eq!(
        history_pointer(after),
        pointer,
        "unmodified historical identity custody must not be cloned"
    );
    after.validate().unwrap();
}
#[test]
fn final_relation_failure_rolls_back_new_and_replaced_wakes_and_body() {
    for replace in [false, true] {
        let (owner, _, _) = fixture(14);
        let mut evidence = owner.evidence().clone();
        let (body, mut wake) = evidence.body.wake(100, "wake/original".into()).unwrap();
        if replace {
            evidence
                .append_wake(body.clone(), wake.clone(), evidence.last_sequence() + 1)
                .unwrap();
            wake = wake.lull("wake/lull".into()).unwrap();
        }
        // Individually valid Wake/Body; only complete biography replay detects
        // that this Wake claims another workload at its original Woke event.
        wake.workset = BodyWorkset::one(ResidentPlot::new(
            "source/other".into(),
            "checked/other".into(),
        ))
        .unwrap();
        body.validate().unwrap();
        wake.validate().unwrap();
        let before = evidence.clone();
        let pointer = history_pointer(&evidence);
        let body_pointer = evidence.body.body_id.as_str().as_ptr();
        assert!(evidence
            .append_wake(body, wake, evidence.last_sequence() + 1)
            .is_err());
        assert_unchanged(&evidence, &before, pointer);
        assert_eq!(evidence.body.body_id.as_str().as_ptr(), body_pointer);
    }
}
#[test]
fn sequence_overflow_and_capacity_refusals_preserve_exact_original_evidence() {
    for capacity in [false, true] {
        let (owner, _, _) = fixture(if capacity { 31 } else { 14 });
        let mut evidence = owner.evidence().clone();
        let (awake, wake) = evidence.body.wake(100, "wake/new".into()).unwrap();
        let wake = wake.lull("wake/lull".into()).unwrap();
        let body = awake
            .retain_after_lull(&wake, "body/retained".into())
            .unwrap();
        let before = evidence.clone();
        let pointer = history_pointer(&evidence);
        let sequence = if capacity {
            evidence.last_sequence() + 1
        } else {
            u64::MAX - 1
        };
        assert_eq!(
            evidence.append_wake(body, wake, sequence),
            Err(if capacity {
                BodyBiographyError::CapacityExhausted
            } else {
                BodyBiographyError::InvalidSequence
            })
        );
        assert_unchanged(&evidence, &before, pointer);
    }
}
#[test]
fn prepared_commit_and_start_refusals_preserve_owner_and_old_history() {
    let (mut owner, host, boot) = fixture(14);
    let stale = proposal(&owner, &host, &boot);
    let other = owner.evidence().body.workset.plots()[1].clone();
    owner.select_plot(&other).unwrap();
    let before = owner.evidence().clone();
    let pointer = history_pointer(owner.evidence());
    assert!(matches!(
        owner.commit_verified_proposal(stale),
        Err(BodyLifecycleSessionError::StaleWorkload)
    ));
    assert_unchanged(owner.evidence(), &before, pointer);
    assert!(owner.realization().is_none());
    let pending = proposal(&owner, &host, &boot);
    owner.commit_verified_proposal(pending).unwrap();
    let (play, mut bad) = start(&owner, &host, &boot);
    bad.sign_ids.reverse();
    let before = owner.evidence().clone();
    let current = owner.realization().cloned();
    let pointer = history_pointer(owner.evidence());
    assert_eq!(
        owner.started(&host, &boot, play, bad),
        Err(BodyLifecycleSessionError::StalePlay)
    );
    assert_unchanged(owner.evidence(), &before, pointer);
    assert_eq!(owner.realization(), current.as_ref());
    // Fresh proposal admission itself reserves five biography records. At
    // this boundary no token is issued, so start cannot enter a capacity-failing
    // partial transaction through the public API.
    let (full, host, boot) = fixture(31);
    let before = full.evidence().clone();
    let pointer = history_pointer(full.evidence());
    assert!(matches!(
        full.prepare_verified_proposal_context(&host, &boot),
        Err(BodyLifecycleSessionError::ArchivePersistenceRequired)
    ));
    assert_unchanged(full.evidence(), &before, pointer);
}
#[test]
fn fourteen_and_fifteen_boot_commits_keep_old_history_and_avoid_whole_clone_peaks() {
    for boots in [14, 15] {
        let (mut owner, host, boot) = fixture(boots);
        let (cloned, clone) = allocation_probe::observe(|| owner.evidence().clone());
        let pointer = history_pointer(owner.evidence());
        let prepared = proposal(&owner, &host, &boot);
        let ((), commit) = allocation_probe::observe(|| {
            owner.commit_verified_proposal(prepared).unwrap();
        });
        assert_eq!(history_pointer(owner.evidence()), pointer);
        owner.evidence().validate().unwrap();
        let (play, wake) = start(&owner, &host, &boot);
        let ((), started) =
            allocation_probe::observe(|| owner.started(&host, &boot, play, wake).unwrap());
        assert_eq!(history_pointer(owner.evidence()), pointer);
        owner.evidence().validate().unwrap();
        println!("boots={boots} clone_retained={} clone_peak={} commit_peak={} commit_requested={} started_peak={} started_requested={}",clone.retained_bytes(),clone.peak_bytes,commit.peak_bytes,commit.requested_bytes,started.peak_bytes,started.requested_bytes);
        assert!(
            commit.peak_bytes < clone.retained_bytes(),
            "commit cloned whole biography: {commit:?}"
        );
        assert!(
            started.peak_bytes < clone.retained_bytes(),
            "start cloned whole biography: {started:?}"
        );
        assert!(
            started.peak_bytes < 8192,
            "CPU fixture start scratch exceeds one control reserve: {started:?}"
        );
        assert_eq!(owner.evidence().membership, cloned.membership);
    }
}

#[test]
fn unchanged_body_wake_matches_replacement_and_rolls_back_late_refusals() {
    for invalid in [false, true] {
        let (mut owner, host, boot) = fixture(14);
        owner
            .commit_verified_proposal(proposal(&owner, &host, &boot))
            .unwrap();
        let (_, mut wake) = start(&owner, &host, &boot);
        let mut evidence = owner.evidence().clone();
        let mut reference = evidence.clone();
        if invalid {
            wake.workset = BodyWorkset::one(ResidentPlot::new(
                "source/other".into(),
                "checked/other".into(),
            ))
            .unwrap();
            wake.validate().unwrap();
        }
        let before = evidence.clone();
        let history = history_pointer(&evidence);
        let body_pointer = evidence.body.body_id.as_str().as_ptr();
        let events_pointer = evidence.body.events.as_ptr();
        let sequence = evidence.last_sequence() + 1;
        let expected = reference.append_wake(reference.body.clone(), wake.clone(), sequence);
        let actual = evidence.append_unchanged_body_wake(wake, sequence);
        assert_eq!(actual, expected);
        assert_eq!(evidence.body.body_id.as_str().as_ptr(), body_pointer);
        assert_eq!(evidence.body.events.as_ptr(), events_pointer);
        if invalid {
            assert_unchanged(&evidence, &before, history);
        } else {
            assert_eq!(evidence, reference);
            evidence.validate().unwrap();
            let before = evidence.clone();
            let wake = evidence.wakes.last().unwrap().clone();
            assert_eq!(
                evidence.append_unchanged_body_wake(wake, sequence + 2),
                Err(BodyBiographyError::DuplicateEvidence)
            );
            assert_unchanged(&evidence, &before, history);
        }
    }
}
#[test]
fn unchanged_body_wake_sequence_refusals_preserve_exact_custody() {
    let (mut owner, host, boot) = fixture(14);
    owner
        .commit_verified_proposal(proposal(&owner, &host, &boot))
        .unwrap();
    let (_, wake) = start(&owner, &host, &boot);
    for sequence in [owner.evidence().last_sequence(), u64::MAX] {
        let mut evidence = owner.evidence().clone();
        let before = evidence.clone();
        let pointer = history_pointer(&evidence);
        assert_eq!(
            evidence.append_unchanged_body_wake(wake.clone(), sequence),
            Err(BodyBiographyError::InvalidSequence)
        );
        assert_unchanged(&evidence, &before, pointer);
    }
}

#[test]
fn cold_terminal_wakes_leave_resident_window_only_with_owned_archive_evidence() {
    let (mut owner, host, mut boot) = fixture(1);
    for number in 1..=3 {
        owner
            .commit_verified_proposal(proposal(&owner, &host, &boot))
            .unwrap();
        let before = owner.evidence().clone();
        assert_eq!(
            owner.seal_terminal_wake_for_checkpoint(),
            Err(BodyLifecycleSessionError::NotLulled)
        );
        assert_eq!(owner.evidence(), &before);
        boot = format!("boot/cold/{number}").into();
        owner = SealedBodyLifecycleSession::resume_here(owner.evidence().clone(), &host, &boot)
            .unwrap();
    }
    assert_eq!(owner.evidence().wakes.len(), 3);
    let wakes = owner.evidence().wakes.clone();
    let body_id = owner.evidence().body_id.clone();
    let sequence = owner.evidence().last_sequence();
    let mut archived = Vec::new();
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        let segment = owner.pending_archives().last().unwrap();
        segment.validate_as_head_of(owner.evidence()).unwrap();
        archived.extend(segment.wakes.clone());
        owner.evidence().validate().unwrap();
        let digest = segment.digest;
        // CPU stand-in for the caller's durable commit acknowledgement.
        owner.acknowledge_archives(digest).unwrap();
    }
    assert_eq!(archived, wakes);
    assert!(owner.evidence().wakes.is_empty());
    assert!(owner.pending_archives().is_empty());
    assert_eq!(owner.evidence().body_id, body_id);
    assert_eq!(owner.evidence().last_sequence(), sequence);
    assert!(!owner.seal_terminal_wake_for_checkpoint().unwrap());
}
