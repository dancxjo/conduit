//! Shared canonical Tour reducers; Root retains effect providers only.
pub(crate) use conduitos_runtime_core::morse::backs::{
    IndicatorBack, LeafBack, MorseBack, TourMorseBack,
};
const _: () = assert!(
    conduit_text::MAXIMUM_MORSE_INPUT_BYTES == conduitos_runtime_core::morse::MAX_INPUT_BYTES
);
const _: () = assert!(
    conduit_text::MAXIMUM_MORSE_PATTERN_BYTES == conduitos_runtime_core::morse::MAX_VALUE_BYTES
);
