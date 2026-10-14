//! Viewer-local calendar projection over explicit timezone resolution truth.

use alloc::string::String;
use conduit_time::{
    CalendarEvent, CalendarEventTime, CivilResolutionChoice, LocalDateTime, NamedTimeZone,
    TemporalInstant, ZonedResolution,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLocalFace {
    pub local: LocalDateTime,
    pub zone: NamedTimeZone,
    pub resolution: CivilResolutionChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedCalendarFace {
    pub event_identity: String,
    pub exact_start: TemporalInstant,
    pub exact_end: TemporalInstant,
    pub event_start: ResolvedLocalFace,
    pub event_end: ResolvedLocalFace,
    pub viewer_start: ResolvedLocalFace,
    pub viewer_end: ResolvedLocalFace,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CalendarFaceRefusal {
    InvalidEvent,
    AllDayHasNoInstantProjection,
    InvalidResolution,
    NonexistentLocalTime,
    EventResolutionMismatch,
    ViewerInstantMismatch,
}

pub fn present_timed_calendar_event(
    event: &CalendarEvent,
    event_start: &ZonedResolution,
    event_end: &ZonedResolution,
    viewer_start: &ZonedResolution,
    viewer_end: &ZonedResolution,
) -> Result<TimedCalendarFace, CalendarFaceRefusal> {
    event
        .validate()
        .map_err(|_| CalendarFaceRefusal::InvalidEvent)?;
    let CalendarEventTime::Timed(span) = &event.time else {
        return Err(CalendarFaceRefusal::AllDayHasNoInstantProjection);
    };
    let span = span.value();
    let exact_start = span.instant.start();
    let exact_end = span.instant.end();
    let event_start = select_resolution(event_start, exact_start)?;
    let event_end = select_resolution(event_end, exact_end)?;
    if event_start.local != span.local_start
        || event_end.local != span.local_end
        || event_start.zone != span.zone
        || event_end.zone != span.zone
    {
        return Err(CalendarFaceRefusal::EventResolutionMismatch);
    }
    let viewer_start =
        select_resolution(viewer_start, exact_start).map_err(map_viewer_resolution_refusal)?;
    let viewer_end =
        select_resolution(viewer_end, exact_end).map_err(map_viewer_resolution_refusal)?;
    Ok(TimedCalendarFace {
        event_identity: event.identity.clone(),
        exact_start: exact_start.clone(),
        exact_end: exact_end.clone(),
        event_start,
        event_end,
        viewer_start,
        viewer_end,
    })
}

fn select_resolution(
    resolution: &ZonedResolution,
    exact: &TemporalInstant,
) -> Result<ResolvedLocalFace, CalendarFaceRefusal> {
    resolution
        .validate()
        .map_err(|_| CalendarFaceRefusal::InvalidResolution)?;
    match resolution {
        ZonedResolution::Unique {
            local,
            zone,
            instant,
        } => resolved(local, zone, instant, exact, CivilResolutionChoice::Unique),
        ZonedResolution::Ambiguous {
            local,
            zone,
            earlier,
            later: _,
        } if conduit_time::TemporalInstant::try_from(earlier.clone()).as_ref() == Ok(exact) => {
            resolved(
                local,
                zone,
                earlier,
                exact,
                CivilResolutionChoice::FoldEarlier,
            )
        }
        ZonedResolution::Ambiguous {
            local,
            zone,
            earlier: _,
            later,
        } => resolved(local, zone, later, exact, CivilResolutionChoice::FoldLater),
        ZonedResolution::Nonexistent { .. } => Err(CalendarFaceRefusal::NonexistentLocalTime),
    }
}

fn resolved(
    local: &conduit_core::LocalDateTime,
    zone: &conduit_core::NamedTimeZone,
    instant: &conduit_core::TemporalInstant,
    exact: &TemporalInstant,
    resolution: CivilResolutionChoice,
) -> Result<ResolvedLocalFace, CalendarFaceRefusal> {
    let instant = TemporalInstant::try_from(instant.clone())
        .map_err(|_| CalendarFaceRefusal::InvalidResolution)?;
    if &instant != exact {
        return Err(CalendarFaceRefusal::EventResolutionMismatch);
    }
    Ok(ResolvedLocalFace {
        local: LocalDateTime::try_from(*local)
            .map_err(|_| CalendarFaceRefusal::InvalidResolution)?,
        zone: NamedTimeZone::try_from(zone.clone())
            .map_err(|_| CalendarFaceRefusal::InvalidResolution)?,
        resolution,
    })
}

fn map_viewer_resolution_refusal(refusal: CalendarFaceRefusal) -> CalendarFaceRefusal {
    match refusal {
        CalendarFaceRefusal::EventResolutionMismatch => CalendarFaceRefusal::ViewerInstantMismatch,
        other => other,
    }
}
