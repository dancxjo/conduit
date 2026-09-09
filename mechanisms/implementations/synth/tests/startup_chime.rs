use conduit_synth::{
    StartupChime, StartupChimeRenderRefusal, STARTUP_CHIME_DURATION_MICROS, STARTUP_CHIME_FRAMES,
};

fn render(block: usize) -> Vec<i16> {
    let mut cue = StartupChime::new();
    let mut result = Vec::new();
    let mut samples = vec![0; block];
    loop {
        let count = cue.render(&mut samples).unwrap();
        if count == 0 {
            break;
        }
        result.extend_from_slice(&samples[..count]);
    }
    assert!(cue.is_finished());
    assert_eq!(cue.frame_cursor(), STARTUP_CHIME_FRAMES);
    result
}

#[test]
fn cue_is_exact_across_host_block_sizes_and_has_a_quiet_finite_tail() {
    let expected = render(256);
    let encoded = expected
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect::<Vec<_>>();
    assert_eq!(
        conduit_core::semantic_digest("conduit.sound/startup-chime-pcm@1", &encoded),
        [
            134, 76, 33, 225, 200, 238, 18, 101, 221, 146, 191, 205, 83, 123, 58, 163, 140, 168,
            52, 77, 189, 207, 163, 129, 67, 0, 67, 3, 63, 125, 102, 121,
        ],
        "a changed canonical cue needs a reviewed score version"
    );
    assert_eq!(expected, render(127));
    assert_eq!(expected, render(1));
    assert_eq!(expected.len(), STARTUP_CHIME_FRAMES as usize);
    assert_eq!(STARTUP_CHIME_DURATION_MICROS, 1_200_000);
    assert_eq!(expected[0], 0);
    assert!(expected[expected.len() - 2_400..]
        .iter()
        .all(|sample| *sample == 0));
    let peak = expected
        .iter()
        .map(|sample| sample.unsigned_abs())
        .max()
        .unwrap();
    assert!((1_000..8_192).contains(&peak), "peak={peak}");
    let largest_step = expected
        .windows(2)
        .map(|pair| (i32::from(pair[1]) - i32::from(pair[0])).abs())
        .max()
        .unwrap();
    assert!(largest_step < 512, "abrupt sample step: {largest_step}");
}

#[test]
fn cancellation_and_refused_blocks_cannot_restart_or_advance_the_cue() {
    let mut cue = StartupChime::new();
    let before = cue;
    assert_eq!(
        cue.render(&mut []),
        Err(StartupChimeRenderRefusal::EmptyBlock)
    );
    let mut too_large = [123; 257];
    assert_eq!(
        cue.render(&mut too_large),
        Err(StartupChimeRenderRefusal::BlockTooLarge)
    );
    assert_eq!(cue, before);
    assert_eq!(too_large, [123; 257]);
    let mut samples = [0; 256];
    cue.render(&mut samples).unwrap();
    cue.cancel();
    let cursor = cue.frame_cursor();
    samples.fill(123);
    assert_eq!(cue.render(&mut samples), Ok(0));
    assert_eq!(cue.frame_cursor(), cursor);
    assert!(cue.is_finished());
    assert_eq!(samples, [123; 256]);
}
