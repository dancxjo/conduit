//! Aggregate audio work is admitted independently of per-block queue capacity.
use super::*;
pub const AUDIO_MAXIMUM_BLOCKS_KEY: &str = "maximum-blocks";
pub const AUDIO_MAXIMUM_MILLIS_KEY: &str = "maximum-audio-millis";
pub const AUDIO_STREAM_MAXIMUM_BLOCKS: u32 = 32_768;
pub const AUDIO_STREAM_MAXIMUM_MILLIS: u32 = 30_000;
fn stream_work_configuration(blocks: u64, millis: u64) -> Vec<KindConfigurationField> {
    vec![
        u64_configuration(
            AUDIO_MAXIMUM_BLOCKS_KEY,
            blocks,
            1,
            u64::from(AUDIO_STREAM_MAXIMUM_BLOCKS),
        ),
        u64_configuration(
            AUDIO_MAXIMUM_MILLIS_KEY,
            millis,
            1,
            u64::from(AUDIO_STREAM_MAXIMUM_MILLIS),
        ),
    ]
}
pub fn audio_play_contract() -> StandardKindContract {
    let mut contract = sink(
        AUDIO_PLAY_KIND,
        "Play audio",
        "Consume bounded timestamped PCM through an exact selected playback resource.",
        vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Input)],
        audio_limits(),
    );
    contract.configuration = stream_work_configuration(3_072, 16_384);
    contract
}

pub fn audio_convert_pcm_profile_contract() -> StandardKindContract {
    let mut configuration = vec![
        u64_configuration(AUDIO_CONVERT_OUTPUT_RATE_KEY, 48_000, 8_000, 192_000),
        text_one_of_configuration(
            AUDIO_CONVERT_OUTPUT_LAYOUT_KEY,
            "stereo-left-right",
            &["mono", "stereo-left-right"],
        ),
    ];
    configuration.extend(stream_work_configuration(2_622, 3_000));
    StandardKindContract {
        kind_id: kind_id(AUDIO_CONVERT_PCM_PROFILE_KIND),
        plain_name: "Convert PCM profile".to_string(),
        summary: "Convert bounded PCM blocks to one explicitly selected sample rate and channel layout."
            .to_string(),
        inputs: vec![port("audio", AUDIO_PCM_INFO_ID, PortDirection::Input)],
        outputs: vec![port("converted", AUDIO_PCM_INFO_ID, PortDirection::Output)],
        configuration,
        limits: audio_limits(),
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")".to_string(),
    }
}
