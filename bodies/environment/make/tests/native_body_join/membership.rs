//! Actual native membership-only path through simulated durable Root custody.
use super::*;
use conduit_body::{
    PortableMembershipAdmissionRequest as Request, SealedBodyLifecycleSession,
    MEMBERSHIP_ADMISSION_REQUEST_SCHEMA,
};
pub(super) fn prove(
    mut owner: SealedBodyLifecycleSession,
    root: &[u8; 32],
    host: &c::HostAdvertisement,
    signer: &SigningKey,
) -> (SealedBodyLifecycleSession, Request) {
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([151; 32]).unwrap();
    let bootstrap = native_body_join::issue(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        secret.clone(),
        [152; 32],
        100,
        1000,
    )
    .unwrap();
    let claim = bootstrap.invitation.claim;
    let mut request = Request {
        schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: c::PROTOCOL_VERSION,
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: "host/initialized-worker".into(),
        boot_id: "boot/initialized-worker".into(),
        offer_generation: c::OfferGeneration(17),
        nonce: claim.nonce,
        signature: [0; 64],
    };
    request.signature = secret.sign(&claim.signing_transcript(
        &request.host_id,
        &request.boot_id,
        request.offer_generation,
    ));
    let bytes = serde_json::to_vec(&request).unwrap();
    assert!(bytes.len() <= 1024);
    match native_body_join::decode_command(&bytes).unwrap() {
        native_body_join::Command::Membership(decoded) => assert_eq!(decoded, request),
        _ => panic!("membership-only request selected another profile"),
    }
    let mut rich: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    rich["host_advertisement"] = serde_json::json!({"capabilities":[{"hidden":"offer"}]});
    assert!(native_body_join::decode_command(&serde_json::to_vec(&rich).unwrap()).is_err());
    assert!(native_body_join::decode_command(&vec![b' '; 1025]).is_err());
    let before = owner.evidence().clone();
    let prior_plan = owner.realization().cloned();
    let manager = native_restart::with_admissions(|m| Ok(serde_json::to_vec(m).unwrap())).unwrap();
    let mut wrong = request.clone();
    wrong.signature[0] ^= 1;
    assert!(native_body_join::membership::admit(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        &wrong,
        101,
    )
    .is_err());
    assert_eq!(owner.evidence(), &before);
    assert_eq!(owner.realization(), prior_plan.as_ref());
    native_restart::with_admissions(|m| {
        assert_eq!(serde_json::to_vec(m).unwrap(), manager);
        Ok(())
    })
    .unwrap();
    let generation = native_restart::generation().unwrap();
    let boxed = refuse_owned(Box::new(owner), root, host, &wrong, 101);
    let boxed = refuse_owned(boxed, root, host, &request, 1001);
    let identity = &*boxed as *const _;
    let mut self_host = request.clone();
    self_host.host_id = host.host_id.clone();
    let boxed = match native_body_join::membership::begin_owned(
        boxed,
        root,
        &host.host_id,
        &host.boot_id,
        &self_host,
        101,
    ) {
        Err(native_body_join::membership::OwnedRefusal::Unstaged(owner, reason)) => {
            assert_eq!(reason, "body-membership-profile");
            assert_eq!(&*owner as *const _, identity);
            owner
        }
        _ => panic!("self-host request entered checkpoint custody"),
    };
    let (reserved, reserve_memory) = crate::allocation_probe::observe(|| {
        native_body_join::membership::PendingCheckpoint::reserve(boxed, *root)
            .ok()
            .unwrap()
    });
    let boxed = reserved.release_unstaged().ok().unwrap();
    let (job, begin_memory) = crate::allocation_probe::observe(|| {
        native_body_join::membership::begin_owned(
            boxed,
            root,
            &host.host_id,
            &host.boot_id,
            &request,
            101,
        )
        .ok()
        .unwrap()
    });
    println!("owned_membership_cpu64: reserve_extra_peak={} reserve_extra_retained={} reserve_stage_extra_peak={} reserve_stage_extra_retained={} body_baseline_excluded=true target_total_or_wcet=false", reserve_memory.peak_bytes, reserve_memory.retained_bytes(), begin_memory.peak_bytes, begin_memory.retained_bytes());
    let job = native_body_join::membership::finish_owned(job)
        .err()
        .unwrap();
    assert!(!native_restart::publication_ready());
    let job = run_owned(job, false);
    let (boxed, draft) = native_body_join::membership::finish_owned(job)
        .ok()
        .unwrap();
    assert_eq!(&*boxed as *const _, identity);
    owner = *boxed;
    assert!(native_restart::publication_ready());
    assert!(native_restart::generation().unwrap() > generation);
    assert_eq!(draft.root(), root);
    let signature = signer.sign(&draft.transcript()).to_bytes();
    let receipt = draft.signed(signature);
    receipt.verify(root, &request).unwrap();
    assert!(!receipt.current_offers_available && !receipt.plan_created && !receipt.play_created);
    assert!(receipt.membership_admitted);
    assert_eq!(owner.realization(), prior_plan.as_ref());
    assert_eq!(owner.evidence().body, before.body);
    let mut response = [0u8; 2048];
    let written = serde_json_core::to_slice(&receipt, &mut response).unwrap();
    conduit_body::PortableMembershipAdmissionReceipt::decode(&response[..written])
        .unwrap()
        .verify(root, &request)
        .unwrap();
    let encoded = serde_json::to_vec(&receipt).unwrap();
    assert!(encoded.len() <= 2048);
    conduit_body::PortableMembershipAdmissionReceipt::decode(&encoded)
        .unwrap()
        .verify(root, &request)
        .unwrap();
    let admitted = owner.evidence().clone();
    assert!(native_body_join::membership::admit(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        &request,
        102,
    )
    .is_err());
    assert_eq!(owner.evidence(), &admitted);
    let boxed = refuse_owned(Box::new(owner), root, host, &request, 102);
    owner = *boxed;
    let mut boxed = Some(Box::new(owner));
    let parked = native_restart::park_box(&mut boxed, root, &host.host_id, &host.boot_id).unwrap();
    owner = *native_restart::restore_parked(parked, &host.host_id, &host.boot_id, root).unwrap();
    assert_eq!(owner.evidence(), &admitted);
    native_restart::with_admissions(|manager| {
        let transcript = [153; 32];
        manager
            .verify_admitted_message(
                &owner.evidence().membership,
                &receipt.credential.part_id,
                &request.host_id,
                &request.boot_id,
                &transcript,
                &secret.sign(&transcript),
            )
            .map_err(|_| "membership-continuity")?;
        Ok(())
    })
    .unwrap();
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([154; 32]).unwrap();
    let claim = native_body_join::issue(
        &mut owner,
        root,
        &host.host_id,
        &host.boot_id,
        secret.clone(),
        [155; 32],
        110,
        1000,
    )
    .unwrap()
    .invitation
    .claim;
    request.invitation_id = claim.invitation_id.clone();
    request.host_id = "host/pending-worker".into();
    request.boot_id = "boot/pending-worker".into();
    request.nonce = claim.nonce;
    request.signature = secret.sign(&claim.signing_transcript(
        &request.host_id,
        &request.boot_id,
        request.offer_generation,
    ));
    (owner, request)
}

struct Access {
    fail: bool,
}
impl native_body_checkpoint::incremental::SectorStorage<native_evidence_region::BodyRegion>
    for Access
{
    fn erase_sector(
        &mut self,
        s: &mut native_evidence_region::BodyRegion,
        bank: u8,
        sector: u8,
    ) -> Result<(), native_body_checkpoint::Refusal> {
        use native_body_checkpoint::Storage;
        if self.fail {
            return Err(native_body_checkpoint::Refusal::Storage);
        }
        if sector == 0 {
            s.erase_bank(bank)?;
        }
        Ok(())
    }
    fn program(
        &mut self,
        s: &mut native_evidence_region::BodyRegion,
        at: u32,
        b: &[u8],
    ) -> Result<(), native_body_checkpoint::Refusal> {
        use native_body_checkpoint::Storage;
        if self.fail {
            Err(native_body_checkpoint::Refusal::Storage)
        } else {
            s.program(at, b)
        }
    }
    fn read(
        &mut self,
        s: &mut native_evidence_region::BodyRegion,
        at: u32,
        b: &mut [u8],
    ) -> Result<(), native_body_checkpoint::Refusal> {
        use native_body_checkpoint::Storage;
        if self.fail {
            Err(native_body_checkpoint::Refusal::Storage)
        } else {
            s.read(at, b)
        }
    }
}
fn run_owned(
    mut job: Box<native_body_join::membership::PendingCheckpoint>,
    fail: bool,
) -> Box<native_body_join::membership::PendingCheckpoint> {
    use native_restart::owned_checkpoint::Phase;
    struct Scheduled(Box<native_body_join::membership::PendingCheckpoint>);
    impl native_flash_jobs::Job for Scheduled {
        fn step(&self) -> u32 {
            self.0.step()
        }
    }
    let gate = native_flash_window::Gate::new(1000, 20, 1000).unwrap();
    let wait = gate.begin_wait(0, 1000).unwrap();
    let mut mailbox = native_flash_jobs::Mailbox::<Scheduled, (), &'static str>::new();
    let (mut submitter, mut executor) = mailbox.split().unwrap();
    let mut access = Access { fail };
    let mut peaks = [0usize; 4];
    loop {
        // CPU fixture bound only; this does not qualify native replay WCET.
        let id = submitter.submit(Scheduled(job), 1000).ok().unwrap();
        assert!(submitter.owner_matches(id, |_| true));
        let mut phase_index = 0;
        let (executed, memory) = crate::allocation_probe::observe(|| {
            executor.execute(&wait, 0, |scheduled, permit| {
                phase_index = match scheduled.0.phase() {
                    Phase::Prepare => 0,
                    Phase::Chunk => 1,
                    Phase::Finish => 2,
                    _ => 3,
                };
                match scheduled.0.phase() {
                    Phase::Prepare => scheduled.0.prepare(permit),
                    Phase::Chunk => scheduled.0.prepare_chunk(permit),
                    Phase::Physical(_) => scheduled.0.advance(&mut access),
                    Phase::Finish => scheduled.0.finish_source(permit),
                    _ => Err("membership-test-phase"),
                }
            })
        });
        executed.unwrap();
        peaks[phase_index] = peaks[phase_index].max(memory.peak_bytes);
        let completed = submitter.take(id).unwrap();
        job = completed.owner.0;
        if completed.result.is_err() {
            assert!(fail);
            assert_eq!(job.phase(), Phase::Failed);
            return job;
        }
        assert_eq!(completed.progress, native_flash_jobs::Progress::AdvancedOne);
        if job.phase() == Phase::Complete {
            println!("owned_membership_cpu64: prepare_extra_peak={} chunk_extra_peak={} finish_extra_peak={} physical_extra_peak={} body_baseline_excluded=true target_total_or_wcet=false",peaks[0],peaks[1],peaks[2],peaks[3]);
            return job;
        }
    }
}
pub(super) fn prove_failed_owned(
    owner: SealedBodyLifecycleSession,
    root: &[u8; 32],
    host: &c::HostAdvertisement,
    request: &Request,
) {
    let owner = Box::new(owner);
    let identity = &*owner as *const _;
    let job = match native_body_join::membership::begin_owned(
        owner,
        root,
        &host.host_id,
        &host.boot_id,
        request,
        120,
    ) {
        Ok(job) => job,
        Err(native_body_join::membership::OwnedRefusal::Unstaged(_, reason)) => {
            panic!("unstaged: {reason}")
        }
        Err(native_body_join::membership::OwnedRefusal::Staged(_, reason)) => {
            panic!("staged: {reason}")
        }
    };
    let job = run_owned(job, true);
    assert_eq!(job.owner() as *const _, identity);
    let job = native_body_join::membership::finish_owned(job)
        .err()
        .unwrap();
    assert!(!native_restart::publication_ready());
    assert!(native_restart::with_admissions(|_| Ok(())).is_err());
    // Retain failed custody throughout the checks; no signing input escaped.
    drop(job);
    assert!(!native_restart::publication_ready());
}

#[test]
#[ignore = "owns Boot-global STORE; run this exact test alone in a fresh process"]
fn owned_membership_physical_failure_retains_unsigned_custody() {
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/root".into(),
        boot_id: "boot/root".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "profile/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let sign = c::bind_sign(&host.host_id, &host.boot_id, None, 1);
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let owner = conduit_environment_runtime::body_image::open_installed_image_sealed(
        image.body.evidence().clone(),
        &host,
    )
    .unwrap();
    let media = native_evidence_region::BodyRegion(std::sync::Arc::new(std::sync::Mutex::new((
        vec![255; 65536],
        true,
    ))));
    native_restart::initialize(media.clone(), owner.evidence().clone()).unwrap();
    let seed = native_restart::select_signer_seed([7; 32]).unwrap();
    let signer = SigningKey::from_bytes(&seed);
    let root_key = signer.verifying_key().to_bytes();
    let retained = native_restart::take_body().unwrap();
    assert!(retained.pristine);
    let mut owner = conduit_body::SealedBodyLifecycleSession::open(retained.evidence).unwrap();
    native_restart::checkpoint(&mut owner, &root_key).unwrap();
    assert!(native_restart::publication_ready());

    let (owner, request) = prove(owner, &root_key, &host, &signer);
    prove_failed_owned(owner, &root_key, &host, &request);
}

fn refuse_owned(
    owner: Box<SealedBodyLifecycleSession>,
    root: &[u8; 32],
    host: &c::HostAdvertisement,
    request: &Request,
    now: u64,
) -> Box<SealedBodyLifecycleSession> {
    let identity = &*owner as *const _;
    let evidence = owner.evidence().clone();
    let manager = native_restart::with_admissions(|m| Ok(serde_json::to_vec(m).unwrap())).unwrap();
    let generation = native_restart::generation();
    let owner = match native_body_join::membership::begin_owned(
        owner,
        root,
        &host.host_id,
        &host.boot_id,
        request,
        now,
    ) {
        Err(native_body_join::membership::OwnedRefusal::Unstaged(owner, reason)) => {
            assert_eq!(reason, "body-membership-admission");
            owner
        }
        _ => panic!("canonical rejection poisoned durable custody"),
    };
    assert_eq!(&*owner as *const _, identity);
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(native_restart::generation(), generation);
    assert!(native_restart::publication_ready());
    native_restart::with_admissions(|m| {
        assert_eq!(serde_json::to_vec(m).unwrap(), manager);
        Ok(())
    })
    .unwrap();
    owner
}
