use conduit_core::{TemporalRelation, TemporalRelationError};
use conduit_form::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_time::{
    NativeTemporalInstant, NativeTemporalScale, TemporalInstantAdapterRefusal,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

fn instant(
    ticks: u64,
    scale: NativeTemporalScale,
    basis: &str,
    resolution_ticks: u64,
    uncertainty_ticks: u64,
) -> NativeTemporalInstant {
    NativeTemporalInstant::new(
        basis.into(),
        resolution_ticks,
        scale,
        ticks,
        uncertainty_ticks,
    )
    .unwrap()
}

fn relation(
    source: &NativeTemporalInstant,
    reference: &NativeTemporalInstant,
) -> Result<TemporalRelation, TemporalInstantAdapterRefusal> {
    let source = conduit_core::TemporalInstant::try_from(source.clone())?;
    let reference = conduit_core::TemporalInstant::try_from(reference.clone())?;
    source.relation_to(&reference).map_err(Into::into)
}

#[test]
fn native_instant_preserves_identity_bounds_and_structured_round_trip() {
    let basis = "b".repeat(MAXIMUM_TEMPORAL_IDENTITY_BYTES);
    let value = instant(
        u64::MAX,
        NativeTemporalScale::Nanoseconds,
        &basis,
        u64::MAX,
        0,
    );
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        NativeTemporalInstant::from_structured(structured).unwrap(),
        value
    );

    for invalid_basis in [
        String::new(),
        "b".repeat(MAXIMUM_TEMPORAL_IDENTITY_BYTES + 1),
    ] {
        assert!(matches!(
            NativeTemporalInstant::new(invalid_basis, 1, NativeTemporalScale::Seconds, 0, 0,),
            Err(NativeBindingRefusal::ViolatedConstraint { .. })
        ));
    }
    assert!(matches!(
        NativeTemporalInstant::new("clock".into(), 0, NativeTemporalScale::Seconds, 0, 0),
        Err(NativeBindingRefusal::ViolatedConstraint { .. })
    ));
}

#[test]
fn core_adapters_are_fallible_and_preserve_every_field() {
    let core = conduit_core::TemporalInstant {
        ticks: 42,
        scale: conduit_core::TemporalScale::Microseconds,
        clock_basis: "clock/source".into(),
        resolution_ticks: 3,
        uncertainty_ticks: 2,
    };
    let native = NativeTemporalInstant::try_from(core.clone()).unwrap();
    assert_eq!(
        conduit_core::TemporalInstant::try_from(native).unwrap(),
        core
    );

    for invalid in [
        conduit_core::TemporalInstant {
            clock_basis: String::new(),
            ..core.clone()
        },
        conduit_core::TemporalInstant {
            clock_basis: "b".repeat(MAXIMUM_TEMPORAL_IDENTITY_BYTES + 1),
            ..core.clone()
        },
        conduit_core::TemporalInstant {
            resolution_ticks: 0,
            ..core
        },
    ] {
        assert_eq!(
            NativeTemporalInstant::try_from(invalid),
            Err(TemporalInstantAdapterRefusal::CoreTemporal(
                TemporalRelationError::InvalidInstant
            ))
        );
    }
}

#[test]
fn native_relation_adapter_matches_core_overflow_and_incomparability() {
    let underflow = instant(0, NativeTemporalScale::Seconds, "clock/a", 1, 1);
    let ordinary = instant(10, NativeTemporalScale::Seconds, "clock/a", 1, 0);
    assert_eq!(
        relation(&underflow, &ordinary),
        Err(TemporalInstantAdapterRefusal::CoreTemporal(
            TemporalRelationError::IntervalOverflow
        ))
    );

    let overflow = instant(u64::MAX, NativeTemporalScale::Seconds, "clock/a", 1, 1);
    assert_eq!(
        relation(&overflow, &ordinary),
        Err(TemporalInstantAdapterRefusal::CoreTemporal(
            TemporalRelationError::IntervalOverflow
        ))
    );

    let other_basis = instant(10, NativeTemporalScale::Seconds, "clock/b", 1, 0);
    let other_scale = instant(10, NativeTemporalScale::Milliseconds, "clock/a", 1, 0);
    for incomparable in [&other_basis, &other_scale] {
        assert_eq!(
            relation(&ordinary, incomparable),
            Err(TemporalInstantAdapterRefusal::CoreTemporal(
                TemporalRelationError::Incomparable
            ))
        );
    }

    let future = instant(20, NativeTemporalScale::Seconds, "clock/a", 1, 0);
    assert_eq!(
        relation(&future, &ordinary),
        Ok(TemporalRelation::Future {
            minimum_ticks: 10,
            maximum_ticks: 10,
        })
    );
}
