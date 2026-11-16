//! Finite poll driver for the ORIGINAL Source actor future, never a scheduler.
use conduit_environment_runtime::publication_face_custody::LocalPublicationFace;
use std::{
    future::Future,
    sync::atomic::Ordering::SeqCst,
    task::{Context, Poll, Waker},
};
fn target_samples() -> u64 {
    let samples = std::env::var("CPU_TARGET_SAMPLES")
        .ok()
        .map(|value| value.parse::<u64>().unwrap())
        .unwrap_or(6);
    assert!(samples >= 6);
    assert!(samples <= crate::workset::sampling::CLOCKS[0].2);
    samples
}
pub fn drive<F: Future>(future: F, face: &LocalPublicationFace) -> bool {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    let start = crate::cpu_time::now_ms();
    let mut completed = false;
    let mut settled = None;
    let target = target_samples();
    let mut plateau = [false; 10];
    let period = crate::workset::sampling::CLOCKS[0].1;
    let poll_budget = ((period * (target + 1) + 5000) / 10) as usize;
    for polls in 0..poll_budget {
        assert!(
            matches!(future.as_mut().poll(&mut cx), Poll::Pending),
            "continuing Source terminated"
        );
        crate::cpu_time::advance(10);
        let status = face.with_retained(|_, status| status).unwrap();
        for (slot, sequence) in [2, 4, 8, 16, 32, 64, 128, 256, 512, target]
            .iter()
            .enumerate()
        {
            if status.source_sequence == *sequence && !plateau[slot] {
                plateau[slot] = true;
                println!(
                    "CPU_SOURCE_PLATEAU sequence={} heap={}",
                    sequence,
                    crate::accounting::used()
                );
            }
        }

        if crate::effects::CAPTURES.load(SeqCst) as u64 >= target
            && crate::effects::frame_count() >= 6
            && crate::cpu_time::now_ms() - start >= 90000
        {
            let status = face.with_retained(|_, status| status).unwrap();
            if status.source_sequence < target {
                continue;
            }
            let first = *settled.get_or_insert(polls);
            if polls < first + 100 {
                continue;
            }
            assert!(status.refusal.is_none(), "publication refusal: {status:?}");
            println!("CPU_SOURCE_PROGRESS polls={} elapsed_ms={} captures={} waits={} programs={} face_sequence={} peer_input_frames=0",polls+1,crate::cpu_time::now_ms()-start,crate::effects::CAPTURES.load(SeqCst),crate::effects::WAITS.load(SeqCst),crate::effects::PROGRAMS.load(SeqCst),status.source_sequence);
            completed = true;
            break;
        }
    }
    assert!(completed,"bounded Source did not keep measuring/displaying: captures={} waits={} programs={} frames={}",crate::effects::CAPTURES.load(SeqCst),crate::effects::WAITS.load(SeqCst),crate::effects::PROGRAMS.load(SeqCst),crate::effects::frame_count());
    assert!(crate::effects::RECEIVES.load(SeqCst) > 0);
    println!("CPU_PEER_ABSENT transport_sends={} receives_pending={} incoming_frames=0 durable_acknowledgments=0",crate::effects::TRANSPORT_SENDS.load(SeqCst),crate::effects::RECEIVES.load(SeqCst));
    if std::env::var_os("CPU_FAIL_FORE_READ").is_some() {
        assert_eq!(crate::effects::RECEIVE_FAILURES.load(SeqCst), 1);
        println!("CPU_FORE_FAILURE read_timeouts=1 original_actor_retirement=true local_effects_continue=true");
    }
    crate::effects::check_frames();
    assert_eq!(crate::effects::CAPTURES.load(SeqCst) as u64, target);
    assert!(crate::effects::PROGRAMS.load(SeqCst) > 0);
    let age = crate::cpu_time::now_ms() - crate::effects::last_frame_ms();
    let fresh = age <= crate::config::RENDERER_PAGE_PERIOD_MS * 5;
    println!(
        "CPU_DISPLAY_LIVENESS last_frame_boot_ms={} age_ms={} fresh={}",
        crate::effects::last_frame_ms(),
        age,
        fresh
    );
    fresh
}
pub fn recover() {
    use conduit_resource_snapshot::append::{
        DurableRecordIndex, PreparedNorRecordResidence, RecordRead, RecordResidenceId,
    };
    use sha2::{Digest, Sha256};
    let storage = crate::equipment::RecordingStorage::from_retained(crate::effects::take_media());
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/nor-record-residence@1");
    for field in ["cpu-factory/body", crate::authority::HOST] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    let mut residence = PreparedNorRecordResidence::prepare(
        storage,
        RecordResidenceId(hash.finalize().into()),
        crate::config::RECORDING_PREFIX_BYTES,
        crate::config::RECORDING_SLOTS,
    )
    .ok()
    .unwrap();
    let recovery = residence.recover().unwrap();
    assert_eq!(recovery.corrupt, 0);
    assert_eq!(recovery.unpublished_or_damaged, 0);
    assert_eq!(recovery.vacant_gaps, 0);
    let contract = conduit_dht11_observation::ObservationContract::prepare().unwrap();
    let mut reader =
        conduit_environment_runtime::recording::PreparedLogReader::prepare_content(&contract)
            .unwrap();
    let mut samples = 0;
    let mut last = 0;
    let mut first_time = None;
    let mut last_time = 0;
    for index in 0..recovery.next_index {
        let RecordRead::Committed { payload, .. } =
            residence.read(DurableRecordIndex(index)).unwrap()
        else {
            panic!("unexpected noncommitted physical slot")
        };
        if let conduit_environment_runtime::recording::LogFrame::Sample {
            sample,
            media_gap_before,
        } = reader.read(payload).unwrap()
        {
            assert!(!media_gap_before);
            let observation =
                conduit_core::validate_canonical_structured_value(sample.observation.unwrap())
                    .unwrap();
            let quantity = |field| {
                let value = observation.record_field(field).unwrap().unwrap();
                conduit_core::Quantity::decode(&value.value_node()[5..]).unwrap()
            };
            assert_eq!(
                quantity("temperature")
                    .compare(
                        conduit_core::Quantity::from_decimal(
                            23 + (samples % 2) as i128,
                            0,
                            conduit_core::Unit::Celsius
                        )
                        .unwrap()
                    )
                    .unwrap(),
                std::cmp::Ordering::Equal
            );
            assert_eq!(
                quantity("humidity")
                    .compare(
                        conduit_core::Quantity::from_decimal(44, 0, conduit_core::Unit::Percent)
                            .unwrap()
                    )
                    .unwrap(),
                std::cmp::Ordering::Equal
            );
            assert_eq!(sample.sequence, last + 1);
            last = sample.sequence;
            first_time.get_or_insert(sample.sampled_at_boot_ms);
            assert!(sample.sampled_at_boot_ms >= last_time);
            last_time = sample.sampled_at_boot_ms;
            samples += 1;
        }
    }
    assert_eq!(samples as u64, target_samples());
    assert_eq!(recovery.committed as u64, target_samples() + 4);
    assert_eq!(recovery.next_index, recovery.committed);
    let context = reader.context().unwrap();
    assert_eq!(context.body, "cpu-factory/body");
    assert_eq!(context.host, crate::authority::HOST);
    assert_eq!(context.boot, crate::authority::BOOT);
    assert_eq!(context.plan, "cpu-factory/plot-plan/0");
    reader.finish().unwrap();
    assert!(last_time - first_time.unwrap() >= 60000);
    println!("CPU_SOURCE_HISTORY committed={} samples={} first_sample_ms={} last_sample_ms={} corruption=0 gaps=0 durable_acknowledgments=0",recovery.committed,samples,first_time.unwrap(),last_time);
}
