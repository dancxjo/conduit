use super::*;

#[test]
fn explicit_format_profiles_keep_one_endpoint_and_validate_real_alsa_options() {
    let _process = microphone_process();
    let mut pools = Vec::new();
    let mut capabilities = Vec::new();
    for (format, device, implementation) in [
        (
            AlsaMicrophoneFormat::ExactHardware,
            "hw:CARD=DSP,DEV=7",
            conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION,
        ),
        (
            AlsaMicrophoneFormat::AlsaPlug,
            "plughw:CARD=DSP,DEV=7",
            conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION,
        ),
    ] {
        // This provider rejects unsupported switches instead of ignoring them.
        // In particular ALSA accepts --file-type, not the old --type spelling.
        let script = format!(
            r#"#!/bin/sh
[ "$1" = --capture ] || exit 40
shift
while [ "$#" -gt 0 ]; do
  case "$1" in
    --device) [ "$2" = '{device}' ] || exit 41 ;;
    --format) [ "$2" = S16_LE ] || exit 42 ;;
    --rate) [ "$2" = 16000 ] || exit 43 ;;
    --channels) [ "$2" = 1 ] || exit 44 ;;
    --samples) [ "$2" = 16 ] || exit 45 ;;
    --file-type) [ "$2" = raw ] || exit 46 ;;
    *) exit 47 ;;
  esac
  shift 2
done
dd if=/dev/zero bs=32 count=1 2>/dev/null
"#
        );
        let (root, discovery) = parsed_fixture(&script, device);
        let selected = discovery.observations[0].clone();
        let mut adapter = discovery
            .initialize_with_format(
                &selected,
                MicrophoneLimits {
                    capture_milliseconds: 1,
                    timeout: Duration::from_secs(2),
                },
                format,
            )
            .unwrap();
        pools.push(adapter.resource_pool_id());
        let offer = adapter.offer();
        assert_eq!(
            offer.implementation.implementation_id.as_str(),
            implementation
        );
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
        capabilities.push(offer.capability_id);
        assert_eq!(adapter.capture(|| false).unwrap(), vec![0; 32]);
        assert_eq!(
            adapter.take_receipt().unwrap().implementation_id,
            implementation
        );
        fs::remove_dir_all(root).unwrap();
    }
    assert_eq!(pools[0], pools[1]);
    assert_ne!(capabilities[0], capabilities[1]);
}

#[test]
fn negotiated_rate_warning_does_not_become_mislabeled_pcm() {
    let _process = microphone_process();
    let (root, discovery) = parsed_fixture("#!/bin/sh\necho 'Warning: rate is not accurate (requested = 16000Hz, got = 48000Hz)' >&2\ndd if=/dev/zero bs=32 count=1 2>/dev/null\n", "rate-warning");
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    assert_eq!(
        adapter.capture(|| false),
        Err(MicrophoneFailure::InexactFormat)
    );
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_recapture_cannot_reuse_an_earlier_receipt() {
    let _process = microphone_process();
    let (root, discovery) = parsed_fixture("#!/bin/sh\nif [ -e \"$0.once\" ]; then exit 1; fi\ntouch \"$0.once\"\ndd if=/dev/zero bs=32 count=1 2>/dev/null\n", "recapture");
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    adapter.capture(|| false).unwrap();
    assert_eq!(
        adapter.capture(|| false),
        Err(MicrophoneFailure::ProviderLost)
    );
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn realistic_capture_durations_obey_shared_pcm_block_bounds_and_contiguity() {
    let _process = microphone_process();
    for milliseconds in [128, 129, 1000, 3000, MAXIMUM_CAPTURE_MILLISECONDS] {
        let bytes = raw_bytes_for_duration(milliseconds);
        let script = format!("#!/bin/sh\ndd if=/dev/zero bs={bytes} count=1 2>/dev/null\n");
        let (root, discovery) = parsed_fixture(&script, "realistic-duration");
        let selected = discovery.observations[0].clone();
        let mut adapter = discovery
            .initialize_with_format(
                &selected,
                MicrophoneLimits {
                    capture_milliseconds: milliseconds,
                    timeout: Duration::from_secs(2),
                },
                AlsaMicrophoneFormat::AlsaPlug,
            )
            .unwrap();
        let encoded = adapter.capture_clip(|| false).unwrap();
        let clip = conduit_audio::decode_pcm_clip(&encoded).unwrap();
        let frames = milliseconds * 16;
        assert_eq!(clip.frame_count, frames);
        assert_eq!(
            clip.blocks.len(),
            (frames as usize).div_ceil(usize::from(conduit_audio::MAXIMUM_PCM_FRAMES_PER_BLOCK))
        );
        assert!(clip.blocks.len() <= conduit_audio::MAXIMUM_PCM_CLIP_BLOCKS);
        let mut next = 0_u64;
        for block in &clip.blocks {
            assert!(block.header.frame_count() <= conduit_audio::MAXIMUM_PCM_FRAMES_PER_BLOCK);
            assert_eq!(block.header.start_frame(), next);
            next += u64::from(block.header.frame_count());
        }
        assert_eq!(next, u64::from(frames));
        assert_eq!(
            adapter.take_receipt().unwrap().raw_pcm_bytes as usize,
            bytes
        );
        fs::remove_dir_all(root).unwrap();
    }
}
