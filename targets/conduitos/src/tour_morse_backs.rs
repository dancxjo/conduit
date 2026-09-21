//! Fixed Backs for the native Tour fan-out/Morse Play.

use crate::text_kernel_backs::{
    LiteralBack, PresentationBack, StepDetails, UpperBack, step_sink, step_transform,
};
use conduit_kernel::RequestId;
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const MORSE_REQUEST: RequestId = RequestId(5);
const INDICATOR_REQUEST: RequestId = RequestId(6);
const LEAF_REQUEST: RequestId = RequestId(7);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MorseBack {
    pub pending: bool,
    pub emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IndicatorBack {
    pub pending: bool,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LeafBack {
    pub maximum_input_bytes: u32,
    pub pending: bool,
    pub emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TourMorseBack {
    Literal(LiteralBack),
    Upper(UpperBack),
    TextPresentation(PresentationBack),
    Morse(MorseBack),
    Indicator(IndicatorBack),
    Leaf(LeafBack),
}

impl StepBack<PORTS> for TourMorseBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Literal(back) => back.step(io),
            Self::Upper(back) => back.step(io),
            Self::TextPresentation(back) => back.step(io),
            Self::Morse(back) => step_transform(
                &mut back.pending,
                &mut back.emitted,
                MORSE_REQUEST,
                conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
                io,
                StepDetails {
                    input: 60,
                    output: Some(61),
                    cancelled: 62,
                    failed: None,
                    lifecycle: 63,
                },
            ),
            Self::Indicator(back) => step_sink(
                &mut back.pending,
                &mut back.complete,
                INDICATOR_REQUEST,
                conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
                io,
                StepDetails {
                    input: 70,
                    output: None,
                    cancelled: 71,
                    failed: None,
                    lifecycle: 72,
                },
            ),
            Self::Leaf(back) => step_transform(
                &mut back.pending,
                &mut back.emitted,
                LEAF_REQUEST,
                back.maximum_input_bytes,
                io,
                StepDetails {
                    input: 80,
                    output: Some(81),
                    cancelled: 82,
                    failed: None,
                    lifecycle: 83,
                },
            ),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Literal(back) => back.state = crate::text_kernel_backs::LiteralState::Cancelled,
            Self::Upper(back) => back.pending = false,
            Self::TextPresentation(back) => back.pending = false,
            Self::Morse(back) => back.pending = false,
            Self::Indicator(back) => back.pending = false,
            Self::Leaf(back) => back.pending = false,
        }
    }
}
