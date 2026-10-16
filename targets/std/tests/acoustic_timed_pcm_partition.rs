//! Actual packed PCM from the authored timed patch is invariant across producer blocks.
#[path = "common_acoustic_timed/output_sink.rs"]
mod output_sink;
#[path = "common_acoustic_timed/pcm_loss_plan.rs"]
mod pcm_loss_plan;
#[path = "common_acoustic_timed/pcm_partition_kernel.rs"]
mod pcm_partition_kernel;
#[test]
fn packed_pcm_is_identical_across_timed_control_blocks_and_held_commit_outputs() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut reference = None;
            let mut source = None;
            for (partitions, gap, hold) in [(&[4][..], 0, 1), (&[1, 2, 1][..], 7, 4)] {
                let prepared = pcm_loss_plan::prepare();
                if let Some(previous) = &source {
                    assert_eq!(&prepared.plan.source_document_id, previous);
                }
                source = Some(prepared.plan.source_document_id.clone());
                let blocks = pcm_partition_kernel::run(
                    prepared.plan,
                    &prepared.front,
                    &prepared.registry,
                    &prepared.items,
                    partitions,
                    gap,
                    hold,
                );
                assert_eq!(blocks.len(), 4);
                for (index, bytes) in blocks.iter().enumerate() {
                    let (header, samples) =
                        conduit_audio::PcmFrameHeader::decode_frame(bytes).unwrap();
                    assert_eq!(header.sample_rate_hz(), 8000);
                    assert_eq!(header.clock_id(), 1);
                    assert_eq!(header.start_frame(), index as u64);
                    assert_eq!(header.frame_count(), 1);
                    assert!(!header.discontinuity());
                    assert_eq!(samples, [24_i16, 104, 263, 425][index].to_le_bytes());
                }
                if let Some(previous) = &reference {
                    assert_eq!(&blocks, previous);
                }
                reference = Some(blocks);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
