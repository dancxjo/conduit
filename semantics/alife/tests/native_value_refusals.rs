use conduit_alife::{LeniaValueRefusal, ReactionDiffusionValueRefusal};
use conduit_plot::rust_binding::NativeRustBinding;

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn lenia_value_refusals_have_one_portable_owner() {
    for refusal in [
        LeniaValueRefusal::InvalidDimensions,
        LeniaValueRefusal::CellCountMismatch,
        LeniaValueRefusal::CellOutOfRange,
        LeniaValueRefusal::InvalidParameters,
        LeniaValueRefusal::GenerationOverflow,
        LeniaValueRefusal::ArithmeticOverflow,
        LeniaValueRefusal::InvalidSeed,
    ] {
        round_trip(refusal);
    }
}

#[test]
fn reaction_diffusion_value_refusals_preserve_payload_truth() {
    for refusal in [
        ReactionDiffusionValueRefusal::InvalidDimensions,
        ReactionDiffusionValueRefusal::CellCountMismatch,
        ReactionDiffusionValueRefusal::ConcentrationOutOfRange,
        ReactionDiffusionValueRefusal::InvalidParameters,
        ReactionDiffusionValueRefusal::WrongFieldIdentity,
        ReactionDiffusionValueRefusal::stale_generation(11, 7).unwrap(),
        ReactionDiffusionValueRefusal::InvalidGenerationCount,
        ReactionDiffusionValueRefusal::GenerationOverflow,
        ReactionDiffusionValueRefusal::work_limit_exceeded(127, 128).unwrap(),
        ReactionDiffusionValueRefusal::ArithmeticOverflow,
    ] {
        round_trip(refusal);
    }
}
