//! Explicit finite capture, distinct from a push-to-talk gesture lifetime.
use crate::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, PortDescriptor, PortDirection,
    PortTemporal,
};
pub const AUDIO_CAPTURE_FINITE_KIND: &str = "audio/capture-finite";
pub const AUDIO_CAPTURE_FINITE_REVISION: &str = "conduit.std/audio-capture-finite@1";
pub const AUDIO_CAPTURE_MILLIS_KEY: &str = "capture-millis";
pub const AUDIO_CAPTURE_BLOCK_FRAMES_KEY: &str = "block-frames";
pub const AUDIO_CAPTURE_FINITE_BLOCK_BYTES: u32 =
    conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32 + 4096;
pub fn audio_capture_finite_contract() -> StandardKindContract {
    let field = |key: &str, default_value, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(default_value),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    };
    StandardKindContract {
        kind_id:kind_id(AUDIO_CAPTURE_FINITE_KIND),
        plain_name:"Capture finite audio".into(),
        summary:"After an explicit capture request, emit a bounded capture-relative PCM flow while acquisition is in progress; no push-to-talk gesture is implied.".into(),
        inputs:vec![PortDescriptor { port_id:port_id("request"),value_kind:kind_id(conduit_text::TEXT_VALUE_KIND),direction:PortDirection::Input,temporal:PortTemporal::Value,abnormal_kind:None }],
        outputs:vec![PortDescriptor { port_id:port_id("audio"),value_kind:kind_id(conduit_audio::AUDIO_PCM_INFO_ID),direction:PortDirection::Output,temporal:PortTemporal::Flow { closes:true },abnormal_kind:None }],
        configuration:vec![field(AUDIO_CAPTURE_MILLIS_KEY,1000,6000),field(AUDIO_CAPTURE_BLOCK_FRAMES_KEY,256,2048)],
        limits:CapabilityLimits { max_active_instances:1,max_queue_items:1,max_queue_bytes:AUDIO_CAPTURE_FINITE_BLOCK_BYTES },
        terminal_behavior:KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required:true,browser_manifestation_honest:false,pico_manifestation_honest:false,
        example:"microphone: audio/capture-finite(capture-millis = 1000, block-frames = 256)".to_string(),
    }
}
