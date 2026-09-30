use crate::TimedPatternRefusal;

impl core::fmt::Display for TimedPatternRefusal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "timed sequence is malformed",
            Self::TooFewEvents => "timed sequence requires at least two events",
            Self::TooManyEvents => "timed sequence exceeds its event bound",
            Self::ReorderedOrDuplicateEvent => "timed sequence events are not strictly ordered",
            Self::IntervalOverflow => "timed sequence interval is not representable",
        })
    }
}
