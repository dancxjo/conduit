use conduit_core::{TemporalInstant, TemporalScale};
use conduit_human::{ExperienceTemporalPolicy, ExperienceTemporalRefusal, ExperienceTemporalRole};

fn at(ticks: u64, uncertainty_ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/experience".into(),
        resolution_ticks: 1,
        uncertainty_ticks,
    }
}

fn policy() -> ExperienceTemporalPolicy {
    ExperienceTemporalPolicy {
        maximum_current_age_ticks: 5,
        maximum_recent_age_ticks: 20,
    }
}

#[test]
fn reviewed_age_thresholds_classify_current_recent_and_stale() {
    assert_eq!(
        policy().classify(&at(100, 0), &at(100, 0)),
        Ok(ExperienceTemporalRole::Current)
    );
    assert_eq!(
        policy().classify(&at(90, 0), &at(100, 0)),
        Ok(ExperienceTemporalRole::Recent)
    );
    assert_eq!(
        policy().classify(&at(70, 0), &at(100, 0)),
        Ok(ExperienceTemporalRole::Stale)
    );
}

#[test]
fn bounded_uncertainty_can_be_current_but_cannot_cross_freshness_boundaries() {
    assert_eq!(
        policy().classify(&at(100, 2), &at(100, 0)),
        Ok(ExperienceTemporalRole::Current)
    );
    assert_eq!(
        policy().classify(&at(94, 2), &at(100, 0)),
        Err(ExperienceTemporalRefusal::IndeterminateAge)
    );
    assert_eq!(
        policy().classify(&at(79, 2), &at(100, 0)),
        Err(ExperienceTemporalRefusal::IndeterminateAge)
    );
}

#[test]
fn future_and_incomparable_observations_refuse_distinctly() {
    assert_eq!(
        policy().classify(&at(101, 0), &at(100, 0)),
        Err(ExperienceTemporalRefusal::FutureObservation)
    );
    let mut other_clock = at(100, 0);
    other_clock.clock_basis = "clock/other".into();
    assert_eq!(
        policy().classify(&other_clock, &at(100, 0)),
        Err(ExperienceTemporalRefusal::IncomparableClock)
    );
}
