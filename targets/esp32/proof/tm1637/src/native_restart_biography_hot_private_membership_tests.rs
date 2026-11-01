//! Actual private copied-source membership phase and restart proof.
use super::*;

#[test]
#[ignore = "owns Boot-global STORE; private real committed source, fresh CPU process only"]
fn private_committed_prefix_owned_membership_survives_reopen() {
    prove_private_membership(false);
}

#[test]
#[ignore = "owns Boot-global STORE; private real source; fresh CPU process only"]
fn private_committed_prefix_pressure_falls_back_before_effects() {
    prove_private_membership(true);
}

fn prove_private_membership(pressure: bool) {
    use crate::native_body_join::{self, membership::job};
    use crate::native_restart::owned_checkpoint::Phase as CheckpointPhase;
    let body: conduit_body::BodyId = serde_json::from_value(serde_json::Value::String(
        std::env::var("CONDUIT_PRIVATE_BODY_ID").unwrap(),
    ))
    .unwrap();
    let host: conduit_core::HostId = std::env::var("CONDUIT_PRIVATE_PRODUCER_HOST")
        .unwrap()
        .into();
    let root = root_pin();
    let raw = std::fs::read(std::env::var("CONDUIT_PRIVATE_BODY_BACKUP").unwrap()).unwrap();
    assert_eq!(raw.len(), 4194304);
    let binding = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    region.program(0, &raw[0x3f0000..0x400000]).unwrap();
    let mut journal = Checkpoint::open(region.clone()).unwrap();
    let mut seed = PrivateSeed([0; 32]);
    let (resident, _) =
        decode_checkpoint(&mut journal, &binding, &body, &mut seed.0, None).unwrap();
    let signer = SigningKey::from_bytes(&seed.0);
    assert!(signer.verifying_key().to_bytes() == root);
    assert!(resident.prefix.is_some());
    let original_prefix = serde_json::to_vec(&resident.prefix).unwrap();
    let original_network = resident.network.as_ref().map(|n| *n.bytes());
    let original_layouts = serde_json::to_vec(&resident.layouts).unwrap();
    crate::native_restart::initialize(region.clone(), resident.retained.evidence).unwrap();
    let retained = crate::native_restart::take_body().unwrap();
    assert!(!retained.pristine);
    let boot: conduit_core::BootId = "boot/copied-real-prefix-join-boot-001".into();
    assert_eq!(boot.as_str().len(), 37);
    let mut owner =
        SealedBodyLifecycleSession::resume_here(retained.evidence, &host, &boot).unwrap();
    while owner.seal_terminal_wake_for_checkpoint().unwrap() {
        crate::native_restart::checkpoint(&mut owner, &root).unwrap();
    }
    owner.seal_membership_history_for_checkpoint().unwrap();
    crate::native_restart::checkpoint(&mut owner, &root).unwrap();
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([181; 32]).unwrap();
    let claim = native_body_join::issue(
        &mut owner,
        &root,
        &host,
        &boot,
        secret.clone(),
        [182; 32],
        100,
        1000,
    )
    .unwrap()
    .invitation
    .claim;
    let mut request = conduit_body::PortableMembershipAdmissionRequest {
        schema: conduit_body::MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        invitation_id: claim.invitation_id.clone(),
        body_id: body.clone(),
        host_id: "host/copied-source-only-member".into(),
        boot_id: "boot/copied-source-only-member".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        nonce: claim.nonce,
        signature: [0; 64],
    };
    request.signature = secret.sign(&claim.signing_transcript(
        &request.host_id,
        &request.boot_id,
        request.offer_generation,
    ));
    let before_recipe = crate::native_restart::with_store(|store| {
        payload::rewrite::probe_frozen_recipe(store, &owner, &root)
    })
    .unwrap();
    let encoding_before = payload::rewrite::canonical_encoding_passes();
    let owner = Box::new(owner);
    let owner_pointer = &*owner as *const _;
    let (mut job, reserve) = crate::allocation_probe::observe(|| {
        job::Job::reserve(owner, root, host.clone(), boot.clone(), request.clone())
            .ok()
            .unwrap()
    });
    let gate = conduit_monotonic_wait::flash_window::Gate::new(1000, 20, 1000).unwrap();
    let mut access = ProfileAccess {
        calls: 0,
        fault: None,
        media: None,
    };
    let mut counts = [0usize; 6];
    let mut peaks = [0usize; 6];
    let mut retained_max = [0usize; 6];
    let mut requested_total = [0usize; 6];
    let mut cache_bytes = 0;
    loop {
        cache_bytes = cache_bytes.max(job.cached_bytes());
        let phase = job.phase();
        if phase == job::Phase::Checkpoint(CheckpointPhase::Complete) {
            break;
        }
        job = job.finish().err().unwrap(); // no receipt before verified adoption
        if counts[0] > 0 {
            job = job.release_unstarted().err().unwrap();
        }
        let index = match phase {
            job::Phase::Start => 0,
            job::Phase::Checkpoint(CheckpointPhase::Prepare) => 1,
            job::Phase::Checkpoint(CheckpointPhase::Capture) => 2,
            job::Phase::Checkpoint(CheckpointPhase::Chunk) => 3,
            job::Phase::Checkpoint(CheckpointPhase::Physical(_)) => 4,
            job::Phase::Checkpoint(CheckpointPhase::Finish) => 5,
            _ => panic!("unexpected membership phase"),
        };
        let wait = gate.begin_wait(0, 1000).unwrap();
        let permit = wait.try_flash(0, 1000).unwrap();
        let ordinal = job.step();
        let calls_before = access.calls;
        let refusal = if pressure && index == 2 {
            Some(crate::allocation_probe::refuse_next_allocation_size(
                job.capture_prefix_bytes().unwrap(),
            ))
        } else {
            None
        };
        let (result, memory) =
            crate::allocation_probe::observe(|| job.advance(&permit, &mut access, 101));
        drop(refusal);
        result.unwrap();
        if index == 2 {
            assert_eq!(
                access.calls, calls_before,
                "Capture must never issue physical effects"
            );
        }
        assert_eq!(job.step(), ordinal + 1);
        counts[index] += 1;
        peaks[index] = peaks[index].max(memory.peak_bytes);
        retained_max[index] = retained_max[index].max(memory.retained_bytes());
        requested_total[index] += memory.requested_bytes;
    }
    let (owner, admitted) = job.finish().ok().unwrap();
    assert_eq!(&*owner as *const _, owner_pointer);
    let signature = signer.sign(&admitted.transcript()).to_bytes();
    let receipt = admitted.signed(signature);
    receipt.verify(&root, &request).unwrap();
    assert!(
        receipt.membership_admitted
            && !receipt.current_offers_available
            && !receipt.plan_created
            && !receipt.play_created
    );
    let encoding_passes = payload::rewrite::canonical_encoding_passes() - encoding_before;
    if pressure {
        assert_eq!(cache_bytes, 0);
        assert!(encoding_passes > 3);
    } else {
        assert_eq!(encoding_passes, 3);
        assert_eq!(peaks[3], 0);
        assert_eq!(requested_total[3], 0);
        assert!(cache_bytes > 0 && cache_bytes <= payload::MAX_BYTES);
    }
    let after_recipe = crate::native_restart::with_store(|store| {
        payload::rewrite::probe_frozen_recipe(store, &owner, &root)
    })
    .unwrap();
    println!("private_actual_source_frozen_recipe_dimensions: prefix_append_suffix_reused_source_output_before={before_recipe:?} after={after_recipe:?} canonical_and_recipe_hashes_equal=true exact_measured_fixture_limits_only=true target_admission=false");
    let mut reopened = Checkpoint::open(region).unwrap();
    let (after, _) = decode_checkpoint(&mut reopened, &binding, &body, &mut seed.0, None).unwrap();
    assert!(after.retained.evidence == *owner.evidence());
    assert!(serde_json::to_vec(&after.prefix).unwrap() == original_prefix);
    assert!(after.network.as_ref().map(|n| *n.bytes()) == original_network);
    assert!(serde_json::to_vec(&after.layouts).unwrap() == original_layouts);
    assert!(SigningKey::from_bytes(&seed.0).verifying_key().to_bytes() == root);
    let transcript = [183; 32];
    after
        .retained
        .admissions
        .unwrap()
        .verify_admitted_message(
            &after.retained.evidence.membership,
            &receipt.credential.part_id,
            &request.host_id,
            &request.boot_id,
            &transcript,
            &secret.sign(&transcript),
        )
        .unwrap();
    let length = reopened.payload_length(&binding).unwrap().unwrap();
    assert!(length <= payload::MAX_BYTES);
    println!("private_actual_prefix_owned_membership: phases_start_prepare_capture_chunk_physical_finish={counts:?} per_phase_peak={peaks:?} per_phase_retained_max={retained_max:?} per_phase_requested_total={requested_total:?} reserve_peak={} reserve_retained={} body_baseline_excluded=true per_step_probes_not_global_live_heap=true canonical_encoding_passes={encoding_passes} cache_retained_capacities_plus_index={cache_bytes} bytes={length} generation={} original_prefix_receipt_preserved=true fixture_receipts=0 forced_capacity_refusal={pressure} cpu64_only=true hardware_admission=false", reserve.peak_bytes, reserve.retained_bytes(), reopened.generation().unwrap());
}
