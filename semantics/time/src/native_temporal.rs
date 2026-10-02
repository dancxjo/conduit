//! Domain behavior layered on Conduitese-owned temporal values.

use crate::{
    CivilTimeRefusal, LocalDate, LocalDateTime, LocalTime, MonotonicClockIdentity,
    MonotonicDuration, MonotonicInstant, MonotonicTimeRefusal, NamedTimeZone, TemporalScale,
};

impl LocalDate {
    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        let maximum =
            days_in_month(self.year(), self.month()).ok_or(CivilTimeRefusal::InvalidDate)?;
        (self.day() <= maximum)
            .then_some(())
            .ok_or(CivilTimeRefusal::InvalidDate)
    }
}

impl LocalTime {
    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        Ok(())
    }
}

impl LocalDateTime {
    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        self.date.validate()?;
        self.time.validate()
    }
}

impl NamedTimeZone {
    pub fn validate(&self) -> Result<(), CivilTimeRefusal> {
        Ok(())
    }
}

impl TryFrom<conduit_core::LocalDate> for LocalDate {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(value: conduit_core::LocalDate) -> Result<Self, Self::Error> {
        Self::new(value.year(), value.month(), value.day())
    }
}

impl TryFrom<LocalDate> for conduit_core::LocalDate {
    type Error = CivilTimeRefusal;

    fn try_from(value: LocalDate) -> Result<Self, Self::Error> {
        Self::new(value.year(), value.month(), value.day())
    }
}

impl TryFrom<conduit_core::LocalTime> for LocalTime {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(value: conduit_core::LocalTime) -> Result<Self, Self::Error> {
        Self::new(
            value.hour(),
            value.minute(),
            value.second(),
            value.nanosecond(),
        )
    }
}

impl TryFrom<LocalTime> for conduit_core::LocalTime {
    type Error = CivilTimeRefusal;

    fn try_from(value: LocalTime) -> Result<Self, Self::Error> {
        Self::new(
            value.hour(),
            value.minute(),
            value.second(),
            value.nanosecond(),
        )
    }
}

impl TryFrom<conduit_core::LocalDateTime> for LocalDateTime {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(value: conduit_core::LocalDateTime) -> Result<Self, Self::Error> {
        Self::new(value.date.try_into()?, value.time.try_into()?)
    }
}

impl TryFrom<LocalDateTime> for conduit_core::LocalDateTime {
    type Error = CivilTimeRefusal;

    fn try_from(value: LocalDateTime) -> Result<Self, Self::Error> {
        Ok(Self {
            date: value.date.try_into()?,
            time: value.time.try_into()?,
        })
    }
}

impl TryFrom<conduit_core::NamedTimeZone> for NamedTimeZone {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(value: conduit_core::NamedTimeZone) -> Result<Self, Self::Error> {
        Self::new(value.identity().into(), value.rule_set().into())
    }
}

impl TryFrom<NamedTimeZone> for conduit_core::NamedTimeZone {
    type Error = CivilTimeRefusal;

    fn try_from(value: NamedTimeZone) -> Result<Self, Self::Error> {
        Self::new(value.identity().clone(), value.rule_set().clone())
    }
}

impl MonotonicClockIdentity {
    pub fn validate(&self) -> Result<(), MonotonicTimeRefusal> {
        Ok(())
    }

    pub fn host_id(&self) -> &str {
        self.host_identity()
    }

    pub fn boot_id(&self) -> &str {
        self.boot_identity()
    }

    pub fn basis_id(&self) -> &str {
        self.basis_identity()
    }
}

impl TryFrom<conduit_core::MonotonicClockIdentity> for MonotonicClockIdentity {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(value: conduit_core::MonotonicClockIdentity) -> Result<Self, Self::Error> {
        Self::new(
            value.basis_id().into(),
            value.boot_id().as_str().into(),
            value.host_id().as_str().into(),
            value.resolution_ticks(),
            value.scale().into(),
            value.uncertainty_ticks(),
        )
    }
}

impl TryFrom<conduit_core::MonotonicInstant> for MonotonicInstant {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(value: conduit_core::MonotonicInstant) -> Result<Self, Self::Error> {
        Self::new(value.clock().clone().try_into()?, value.ticks())
    }
}

impl MonotonicInstant {
    pub fn validate(&self) -> Result<(), MonotonicTimeRefusal> {
        self.clock().validate()
    }

    pub fn after(&self, duration: MonotonicDuration) -> Result<Self, MonotonicTimeRefusal> {
        if duration.scale() != *self.clock().scale() {
            return Err(MonotonicTimeRefusal::DifferentScale);
        }
        let ticks = self
            .ticks()
            .checked_add(duration.ticks())
            .ok_or(MonotonicTimeRefusal::Overflow)?;
        Self::new(self.clock().clone(), ticks).map_err(|_| MonotonicTimeRefusal::Overflow)
    }
}

fn days_in_month(year: i32, month: u8) -> Option<u8> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[allow(dead_code)]
const fn _scale_is_portable(_: TemporalScale) {}
