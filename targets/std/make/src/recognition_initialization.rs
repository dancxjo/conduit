//! Exact, effect-free declaration of an existing bounded Whisper clip provider.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};

pub fn whisper_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    let number = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("executable", 4096),
            text("executable_sha256", 64),
            text("model", 4096),
            text("model_sha256", 64),
            number("model_bytes", 1, 256 * 1024 * 1024),
            text("language_coverage", 4096),
            text("language_coverage_sha256", 64),
            number("threads", 1, 32),
            number("timeout_milliseconds", 1, 120000),
        ],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: "speech/recognizer".into(),
        implementation_id: conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION.into(),
        implementation_revision: 2,
        target_patterns: vec![
            "std/x86_64/computer".into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
        ],
        prerequisites: Vec::new(),
        build_feature: None,
    }
}
