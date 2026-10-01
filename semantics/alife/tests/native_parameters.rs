use conduit_alife::{
    GrayScottParameters, LeniaBoundary, LeniaParameters, LeniaRefusal, LeniaValueRefusal,
    ReactionDiffusionRefusal, ReactionDiffusionValueRefusal, LENIA_Q16_ONE,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn lenia_parameters_round_trip_exact_native_bounds_and_retain_owner_validation() {
    for parameters in [
        LeniaParameters::new(1, 0, 1, 0, 1, 1, LeniaBoundary::Wrap).unwrap(),
        LeniaParameters::new(
            16,
            LENIA_Q16_ONE,
            LENIA_Q16_ONE,
            LENIA_Q16_ONE,
            LENIA_Q16_ONE,
            LENIA_Q16_ONE,
            LeniaBoundary::Wrap,
        )
        .unwrap(),
        LeniaParameters::ORBIUM,
    ] {
        parameters.validate().unwrap();
        let structured = parameters.into_structured().unwrap();
        assert_eq!(
            LeniaParameters::from_structured(structured).unwrap(),
            parameters
        );
    }

    assert!(LeniaParameters::new(0, 0, 1, 0, 1, 1, LeniaBoundary::Wrap).is_err());
    assert!(LeniaParameters::new(17, 0, 1, 0, 1, 1, LeniaBoundary::Wrap).is_err());
    let mut bypassed = LeniaParameters::ORBIUM;
    bypassed.dt_q16 = 0;
    assert_eq!(
        bypassed.validate(),
        Err(LeniaRefusal::Value(LeniaValueRefusal::InvalidParameters))
    );
}

#[test]
fn gray_scott_parameters_round_trip_bounds_and_own_relational_law() {
    for parameters in [
        GrayScottParameters::new(1, 1, 0, 0, 1).unwrap(),
        GrayScottParameters::new(1_000_000, 1_000_000, 0, 1_000_000, 1_000_000).unwrap(),
        GrayScottParameters::REFERENCE,
    ] {
        parameters.validate().unwrap();
        let structured = parameters.into_structured().unwrap();
        assert_eq!(
            GrayScottParameters::from_structured(structured).unwrap(),
            parameters
        );
    }

    assert!(GrayScottParameters::new(0, 1, 0, 0, 1).is_err());
    assert!(GrayScottParameters::new(1, 1, 0, 0, 1_000_001).is_err());
    assert!(GrayScottParameters::new(1, 1, 600_000, 500_001, 1).is_err());
    let relationally_invalid = GrayScottParameters {
        diffusion_u_ppm: 1,
        diffusion_v_ppm: 1,
        feed_ppm: 600_000,
        kill_ppm: 500_001,
        time_step_ppm: 1,
    };
    assert_eq!(
        relationally_invalid.validate(),
        Err(ReactionDiffusionRefusal::Value(
            ReactionDiffusionValueRefusal::InvalidParameters
        ))
    );
}
