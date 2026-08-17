use conduit_core::{
    TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow, TemporalWindowPosition,
    TemporalWindowRefusal,
};

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "unix-epoch".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

#[test]
fn closed_window_classifies_both_boundaries_within() {
    let window = TemporalWindow::new(
        instant(10),
        TemporalBoundary::Inclusive,
        instant(20),
        TemporalBoundary::Inclusive,
    )
    .unwrap();
    assert_eq!(
        window.classify(&instant(9)),
        Ok(TemporalWindowPosition::Before)
    );
    assert_eq!(
        window.classify(&instant(10)),
        Ok(TemporalWindowPosition::Within)
    );
    assert_eq!(
        window.classify(&instant(20)),
        Ok(TemporalWindowPosition::Within)
    );
    assert_eq!(
        window.classify(&instant(21)),
        Ok(TemporalWindowPosition::After)
    );
}

#[test]
fn open_boundaries_are_outside_without_changing_the_instants() {
    let window = TemporalWindow::new(
        instant(10),
        TemporalBoundary::Exclusive,
        instant(20),
        TemporalBoundary::Exclusive,
    )
    .unwrap();
    assert_eq!(
        window.classify(&instant(10)),
        Ok(TemporalWindowPosition::Before)
    );
    assert_eq!(
        window.classify(&instant(11)),
        Ok(TemporalWindowPosition::Within)
    );
    assert_eq!(
        window.classify(&instant(20)),
        Ok(TemporalWindowPosition::After)
    );
}

#[test]
fn uncertain_overlap_is_indeterminate_not_inside() {
    let window = TemporalWindow::new(
        instant(10),
        TemporalBoundary::Inclusive,
        instant(20),
        TemporalBoundary::Inclusive,
    )
    .unwrap();
    let mut candidate = instant(9);
    candidate.uncertainty_ticks = 2;
    assert_eq!(
        window.classify(&candidate),
        Ok(TemporalWindowPosition::Indeterminate)
    );
}

#[test]
fn reversed_overlapping_and_empty_boundaries_refuse_distinctly() {
    assert_eq!(
        TemporalWindow::new(
            instant(20),
            TemporalBoundary::Inclusive,
            instant(10),
            TemporalBoundary::Inclusive,
        ),
        Err(TemporalWindowRefusal::Reversed)
    );
    let mut uncertain_start = instant(10);
    uncertain_start.uncertainty_ticks = 2;
    assert_eq!(
        TemporalWindow::new(
            uncertain_start,
            TemporalBoundary::Inclusive,
            instant(11),
            TemporalBoundary::Inclusive,
        ),
        Err(TemporalWindowRefusal::IndeterminateBoundaryOrder)
    );
    assert_eq!(
        TemporalWindow::new(
            instant(10),
            TemporalBoundary::Exclusive,
            instant(10),
            TemporalBoundary::Inclusive,
        ),
        Err(TemporalWindowRefusal::Empty)
    );
}

#[test]
fn incomparable_candidate_refuses_instead_of_becoming_outside() {
    let window = TemporalWindow::new(
        instant(10),
        TemporalBoundary::Inclusive,
        instant(20),
        TemporalBoundary::Inclusive,
    )
    .unwrap();
    let mut other = instant(15);
    other.clock_basis = "boot/monotonic".into();
    assert_eq!(
        window.classify(&other),
        Err(TemporalWindowRefusal::Incomparable)
    );
}
