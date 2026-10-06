use super::*;

fn one_frame() -> Vec<u8> {
    let header = PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        SAMPLE_RATE_HZ,
        PcmChannelLayout::StereoLeftRight,
        1,
        9,
        0,
        false,
    )
    .unwrap();
    let mut frame = header.encode().to_vec();
    frame.extend_from_slice(&[0; 4]);
    frame
}

#[test]
fn exact_plays_publish_separately_and_refuse_stale_or_full_pool() {
    let root = std::env::temp_dir().join(format!("conduit-per-play-wav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let pool = WavArtifactSelection::per_play_root(
        &root,
        BootId::from("boot/wav-test"),
        OfferGeneration(1),
    )
    .unwrap();
    let plan = PlanId::from("plan/wav-test");
    let placement = PlacementId::from("placement/wav-test");
    let mut locators = Vec::new();
    for index in 0..MAX_RETAINED_ARTIFACTS {
        let play = ActivePlayId::from(format!("play/wav-{index}").as_str());
        let exact = pool.for_play(&plan, &play, &placement).unwrap();
        let locator = exact.locator().unwrap();
        assert!(
            pool.for_play(&plan, &play, &placement).is_err(),
            "reserved Play identity cannot be reused"
        );
        let mut session = WavArtifactSession::prepare(exact);
        session.write_frame(&one_frame()).unwrap();
        session.finish().unwrap();
        assert_eq!(session.locator().as_deref(), Some(locator.as_str()));
        drop(session);
        assert!(std::path::Path::new(&locator).exists());
        assert!(
            pool.for_play(&plan, &play, &placement).is_err(),
            "published Play identity cannot be reused"
        );
        locators.push(locator);
    }
    assert_eq!(locators.len(), MAX_RETAINED_ARTIFACTS);
    assert!(!pool.is_unpublished());
    assert!(pool
        .for_play(&plan, &ActivePlayId::from("play/wav-extra"), &placement)
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancelled_play_releases_reservation_without_publishing_partial() {
    let root = std::env::temp_dir().join(format!("conduit-cancelled-wav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let pool = WavArtifactSelection::per_play_root(
        &root,
        BootId::from("boot/wav-test"),
        OfferGeneration(1),
    )
    .unwrap();
    let plan = PlanId::from("plan/wav-test");
    let play = ActivePlayId::from("play/wav-cancel");
    let placement = PlacementId::from("placement/wav-test");
    let exact = pool.for_play(&plan, &play, &placement).unwrap();
    let locator = exact.locator().unwrap();
    {
        let mut session = WavArtifactSession::prepare(exact);
        session.write_frame(&one_frame()).unwrap();
    }
    assert!(!std::path::Path::new(&locator).exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    assert!(pool.for_play(&plan, &play, &placement).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_preparations_cannot_reserve_the_same_play_twice() {
    let root = std::env::temp_dir().join(format!("conduit-racing-wav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let pool = WavArtifactSelection::per_play_root(
        &root,
        BootId::from("boot/wav-test"),
        OfferGeneration(1),
    )
    .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let pool = pool.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                pool.for_play(
                    &PlanId::from("plan/wav-test"),
                    &ActivePlayId::from("play/wav-race"),
                    &PlacementId::from("placement/wav-test"),
                )
            })
        })
        .collect::<Vec<_>>();
    let reservations = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        reservations.iter().filter(|result| result.is_ok()).count(),
        1
    );
    drop(reservations);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn stale_quota_lock_withdraws_route_without_removing_evidence() {
    let root = std::env::temp_dir().join(format!("conduit-stale-lock-wav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let pool = WavArtifactSelection::per_play_root(
        &root,
        BootId::from("boot/wav-test"),
        OfferGeneration(1),
    )
    .unwrap();
    std::fs::write(root.join("retained.wav"), b"prior evidence").unwrap();
    std::fs::create_dir(root.join(".quota-lock")).unwrap();
    assert!(!pool.is_unpublished());
    assert!(pool
        .for_play(
            &PlanId::from("plan/wav-test"),
            &ActivePlayId::from("play/wav-stale-lock"),
            &PlacementId::from("placement/wav-test"),
        )
        .is_err());
    assert_eq!(
        std::fs::read(root.join("retained.wav")).unwrap(),
        b"prior evidence"
    );
    std::fs::remove_dir(root.join(".quota-lock")).unwrap();
    assert!(pool.is_unpublished());
    std::fs::remove_dir_all(root).unwrap();
}

fn selection(root: &Path) -> WavArtifactSelection {
    WavArtifactSelection::new(
        root.join("answer.wav"),
        BootId::from("boot/wav-test"),
        OfferGeneration(1),
    )
    .unwrap()
}

#[test]
fn malformed_or_unfinished_pcm_never_creates_an_artifact() {
    let root = std::env::temp_dir().join(format!(
        "conduit-wav-artifact-refusal-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let destination = root.join("answer.wav");
    let mut session = WavArtifactSession::prepare(selection(&root));
    assert!(session.write_frame(b"not-pcm").is_err());
    assert!(session.finish().is_err());
    assert!(!destination.exists());
    drop(session);
    assert!(!destination.exists());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn bounded_long_artifact_is_incremental_and_unfinished_output_is_not_published() {
    let root = std::env::temp_dir().join(format!("conduit-long-wav-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut session = WavArtifactSession::prepare_bounded(selection(&root), 4000, 20000).unwrap();
    let mut bytes = Vec::new();
    for block in 0..3750_u64 {
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            48000,
            PcmChannelLayout::StereoLeftRight,
            256,
            9,
            block * 256,
            false,
        )
        .unwrap();
        bytes.clear();
        bytes.extend_from_slice(&header.encode());
        bytes.resize(bytes.len() + 1024, 0);
        session.write_frame(&bytes).unwrap();
    }
    assert!(!root.join("answer.wav").exists());
    assert_eq!(
        std::fs::metadata(&session.temporary).unwrap().len(),
        44 + 20 * 48000 * 4
    );
    session.finish().unwrap();
    assert_eq!(session.report().frames, 20 * 48000);
    assert!(session.content_sha256().is_some());
    drop(session);
    std::fs::remove_file(root.join("answer.wav")).unwrap();
    let mut unfinished = WavArtifactSession::prepare(selection(&root));
    let header = PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        48000,
        PcmChannelLayout::StereoLeftRight,
        1,
        9,
        0,
        false,
    )
    .unwrap();
    let mut frame = header.encode().to_vec();
    frame.extend_from_slice(&[0; 4]);
    unfinished.write_frame(&frame).unwrap();
    assert!(
        unfinished.write_frame(&frame).is_err(),
        "repeated frame is discontinuous"
    );
    assert!(
        unfinished.finish().is_err(),
        "a failed write cannot become a completed artifact"
    );
    assert!(!root.join("answer.wav").exists());
    drop(unfinished);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir(root).unwrap();
}
