use conduit_presentation::{present_timed_calendar_event, CalendarPresentationRefusal};
use conduit_time::{
    CalendarEvent, CalendarEventTime, LocalDate, LocalDateTime, LocalTime, NamedTimeZone,
    TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow, TimedCalendarSpan,
    ZonedResolution,
};

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "unix-utc".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn zone(identity: &str) -> NamedTimeZone {
    NamedTimeZone::new(identity.into(), "tzdb/2026a".into()).unwrap()
}

fn local(hour: u8, minute: u8) -> LocalDateTime {
    LocalDateTime::new(
        LocalDate::new(2026, 11, 1).unwrap(),
        LocalTime::new(hour, minute, 0, 0).unwrap(),
    )
}

fn event() -> CalendarEvent {
    CalendarEvent {
        identity: "event/cross-zone".into(),
        title: "Cross-zone meeting".into(),
        description: String::new(),
        location: String::new(),
        time: CalendarEventTime::Timed(TimedCalendarSpan {
            local_start: local(1, 30),
            local_end: local(2, 0),
            zone: zone("America/Los_Angeles"),
            instant: TemporalWindow::new(
                instant(1_793_515_800),
                TemporalBoundary::Inclusive,
                instant(1_793_517_600),
                TemporalBoundary::Exclusive,
            )
            .unwrap(),
        }),
        participants: vec![],
        recurrence: None,
        reminders: vec![],
    }
}

fn unique(local: LocalDateTime, zone: NamedTimeZone, instant: TemporalInstant) -> ZonedResolution {
    ZonedResolution::Unique {
        local,
        zone,
        instant,
    }
}

#[test]
fn same_exact_instants_render_in_event_and_viewer_zones_without_identity_change() {
    let event = event();
    let event_zone = zone("America/Los_Angeles");
    let viewer_zone = zone("Europe/London");
    let event_start = ZonedResolution::Ambiguous {
        local: local(1, 30),
        zone: event_zone.clone(),
        earlier: instant(1_793_515_800),
        later: instant(1_793_519_400),
    };
    let event_end = unique(local(2, 0), event_zone, instant(1_793_517_600));
    let viewer_start = unique(local(8, 30), viewer_zone.clone(), instant(1_793_515_800));
    let viewer_end = unique(local(9, 0), viewer_zone, instant(1_793_517_600));

    let rendered =
        present_timed_calendar_event(&event, &event_start, &event_end, &viewer_start, &viewer_end)
            .unwrap();
    assert_eq!(rendered.event_identity, event.identity);
    assert_eq!(rendered.exact_start, instant(1_793_515_800));
    assert_eq!(rendered.exact_end, instant(1_793_517_600));
    assert_eq!(rendered.event_start.local, local(1, 30));
    assert_eq!(rendered.viewer_start.local, local(8, 30));
    assert_eq!(
        rendered.event_start.resolution,
        conduit_time::CivilResolutionChoice::FoldEarlier
    );
    assert_eq!(rendered.viewer_start.zone.identity(), "Europe/London");
}

#[test]
fn stale_or_invented_viewer_resolution_cannot_rewrite_the_instant() {
    let event = event();
    let event_zone = zone("America/Los_Angeles");
    let viewer_zone = zone("Europe/London");
    let result = present_timed_calendar_event(
        &event,
        &unique(local(1, 30), event_zone.clone(), instant(1_793_515_800)),
        &unique(local(2, 0), event_zone, instant(1_793_517_600)),
        &unique(local(8, 30), viewer_zone.clone(), instant(999)),
        &unique(local(9, 0), viewer_zone, instant(1_793_517_600)),
    );
    assert_eq!(
        result,
        Err(CalendarPresentationRefusal::ViewerInstantMismatch)
    );
}

#[test]
fn nonexistent_viewer_time_and_all_day_projection_refuse_distinctly() {
    let event = event();
    let event_zone = zone("America/Los_Angeles");
    let viewer_zone = zone("Europe/London");
    let nonexistent = ZonedResolution::Nonexistent {
        local: local(1, 30),
        zone: viewer_zone.clone(),
        gap_before: instant(1_793_515_799),
        gap_after: instant(1_793_515_800),
    };
    assert_eq!(
        present_timed_calendar_event(
            &event,
            &unique(local(1, 30), event_zone.clone(), instant(1_793_515_800)),
            &unique(local(2, 0), event_zone, instant(1_793_517_600)),
            &nonexistent,
            &unique(local(9, 0), viewer_zone, instant(1_793_517_600)),
        ),
        Err(CalendarPresentationRefusal::NonexistentLocalTime)
    );

    let mut all_day = event;
    all_day.time = CalendarEventTime::AllDay {
        start: LocalDate::new(2026, 11, 1).unwrap(),
        end_exclusive: LocalDate::new(2026, 11, 2).unwrap(),
    };
    assert_eq!(
        present_timed_calendar_event(
            &all_day,
            &nonexistent,
            &nonexistent,
            &nonexistent,
            &nonexistent,
        ),
        Err(CalendarPresentationRefusal::AllDayHasNoInstantProjection)
    );
}
