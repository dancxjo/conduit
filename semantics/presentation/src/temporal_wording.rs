//! Finite fixed-English wording for already-derived temporal relations.

use alloc::format;
use alloc::string::String;

use crate::{PresentationTemporalFact, TemporalRelation, TemporalScale};

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const NANOS_PER_MINUTE: u128 = 60 * NANOS_PER_SECOND;
const NANOS_PER_HOUR: u128 = 60 * NANOS_PER_MINUTE;
const NANOS_PER_DAY: u128 = 24 * NANOS_PER_HOUR;

/// Render one validated fact without acquiring a clock or changing its identity.
pub fn format_relative_time(fact: &PresentationTemporalFact) -> String {
    match fact.relation {
        TemporalRelation::Present => "just now".into(),
        TemporalRelation::Indeterminate => "time overlaps reference".into(),
        TemporalRelation::Past {
            minimum_ticks,
            maximum_ticks,
        } => format_interval(
            minimum_ticks,
            maximum_ticks,
            fact.source.scale,
            Direction::Past,
        ),
        TemporalRelation::Future {
            minimum_ticks,
            maximum_ticks,
        } => format_interval(
            minimum_ticks,
            maximum_ticks,
            fact.source.scale,
            Direction::Future,
        ),
    }
}

#[derive(Copy, Clone)]
enum Direction {
    Past,
    Future,
}

fn format_interval(
    minimum_ticks: u64,
    maximum_ticks: u64,
    scale: TemporalScale,
    direction: Direction,
) -> String {
    let nanos_per_tick = match scale {
        TemporalScale::Seconds => NANOS_PER_SECOND,
        TemporalScale::Milliseconds => 1_000_000,
        TemporalScale::Microseconds => 1_000,
        TemporalScale::Nanoseconds => 1,
    };
    let minimum_nanos = u128::from(minimum_ticks) * nanos_per_tick;
    let maximum_nanos = u128::from(maximum_ticks) * nanos_per_tick;
    let unit = Unit::for_maximum(maximum_nanos);
    let minimum = minimum_nanos / unit.nanos();
    let maximum = ceiling_division(maximum_nanos, unit.nanos());

    if minimum == maximum {
        let quantity = format!("{minimum} {}", unit.label(minimum));
        match direction {
            Direction::Past => format!("{quantity} ago"),
            Direction::Future => format!("in {quantity}"),
        }
    } else {
        match direction {
            Direction::Past => {
                format!(
                    "between {minimum} and {maximum} {} ago",
                    unit.label(maximum)
                )
            }
            Direction::Future => {
                format!("in {minimum} to {maximum} {}", unit.label(maximum))
            }
        }
    }
}

#[derive(Copy, Clone)]
enum Unit {
    Second,
    Minute,
    Hour,
    Day,
}

impl Unit {
    fn for_maximum(maximum_nanos: u128) -> Self {
        if maximum_nanos < NANOS_PER_MINUTE {
            Self::Second
        } else if maximum_nanos < NANOS_PER_HOUR {
            Self::Minute
        } else if maximum_nanos < 48 * NANOS_PER_HOUR {
            Self::Hour
        } else {
            Self::Day
        }
    }

    fn nanos(self) -> u128 {
        match self {
            Self::Second => NANOS_PER_SECOND,
            Self::Minute => NANOS_PER_MINUTE,
            Self::Hour => NANOS_PER_HOUR,
            Self::Day => NANOS_PER_DAY,
        }
    }

    fn label(self, quantity: u128) -> &'static str {
        match (self, quantity == 1) {
            (Self::Second, true) => "second",
            (Self::Second, false) => "seconds",
            (Self::Minute, true) => "minute",
            (Self::Minute, false) => "minutes",
            (Self::Hour, true) => "hour",
            (Self::Hour, false) => "hours",
            (Self::Day, true) => "day",
            (Self::Day, false) => "days",
        }
    }
}

fn ceiling_division(value: u128, divisor: u128) -> u128 {
    value / divisor + u128::from(!value.is_multiple_of(divisor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PresentationTemporalRole, TemporalInstant};

    fn fact(scale: TemporalScale, relation: TemporalRelation) -> PresentationTemporalFact {
        PresentationTemporalFact {
            subject: "part/example".into(),
            role: PresentationTemporalRole::Observation,
            sign_id: None,
            source: TemporalInstant {
                ticks: 0,
                scale,
                clock_basis: "clock/example".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            reference: "reference/example".into(),
            relation,
        }
    }

    #[test]
    fn fixed_states_do_not_manufacture_precision() {
        assert_eq!(
            format_relative_time(&fact(TemporalScale::Seconds, TemporalRelation::Present)),
            "just now"
        );
        assert_eq!(
            format_relative_time(&fact(
                TemporalScale::Seconds,
                TemporalRelation::Indeterminate
            )),
            "time overlaps reference"
        );
    }

    #[test]
    fn scale_and_granularity_boundaries_are_finite() {
        let vectors = [
            (TemporalScale::Seconds, 59, "59 seconds ago"),
            (TemporalScale::Milliseconds, 60_000, "1 minute ago"),
            (TemporalScale::Microseconds, 3_600_000_000, "1 hour ago"),
            (
                TemporalScale::Nanoseconds,
                172_800_000_000_000,
                "2 days ago",
            ),
        ];
        for (scale, ticks, expected) in vectors {
            assert_eq!(
                format_relative_time(&fact(
                    scale,
                    TemporalRelation::Past {
                        minimum_ticks: ticks,
                        maximum_ticks: ticks,
                    }
                )),
                expected
            );
        }
    }

    #[test]
    fn uncertain_bounds_round_outward_and_keep_direction() {
        assert_eq!(
            format_relative_time(&fact(
                TemporalScale::Milliseconds,
                TemporalRelation::Past {
                    minimum_ticks: 120_001,
                    maximum_ticks: 179_999,
                }
            )),
            "between 2 and 3 minutes ago"
        );
        assert_eq!(
            format_relative_time(&fact(
                TemporalScale::Seconds,
                TemporalRelation::Future {
                    minimum_ticks: 120,
                    maximum_ticks: 180,
                }
            )),
            "in 2 to 3 minutes"
        );
    }
}
