use conduit_core::{
    AuthorityBinding, AuthorityContractId, AuthorityGrantId, BootId, CapabilityId, HostId,
    HostOperationContractId, KindId,
};
use conduit_semantic_catalog::{REMINDER_DELIVERY_AUTHORITY, REMINDER_DELIVER_KIND};
use conduit_std_host::hosted_reminder::{
    deliver_ready_reminder, HostedReminderAdapter, ReminderAdapterError, ReminderDeliveryRefusal,
};
use conduit_std_offers::REMINDER_DELIVER_OPERATION;
use conduit_time::{
    CivilTrigger, ClockChangeBehavior, MissedOccurrencePolicy, NamedTimeZone, OccurrenceInstant,
    RecurrenceOccurrence, ReminderOccurrence, ScheduledIntent, ScheduledOccurrenceDecision,
    TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow, TriggerObservation,
    TriggerProfile,
};

#[test]
fn civil_reminder_fires_once_without_executing_or_mutating_the_event() {
    let start = wall(1_000);
    let end = wall(1_100);
    let scheduled = ScheduledIntent {
        identity: "scheduled/reminder/meeting#0".into(),
        occurrence: RecurrenceOccurrence {
            identity: "recurrence/meeting-reminder/occurrence/0".into(),
            recurrence_identity: "recurrence/meeting-reminder".into(),
            ordinal: 0,
            at: OccurrenceInstant::Wall(start.clone()),
        },
        trigger: TriggerProfile::Civil(CivilTrigger {
            window: TemporalWindow::new(
                start,
                TemporalBoundary::Inclusive,
                end,
                TemporalBoundary::Inclusive,
            )
            .unwrap(),
            zone: NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into()).unwrap(),
            clock_change: ClockChangeBehavior::ReevaluateWindow,
        }),
        missed: MissedOccurrencePolicy::Skip,
        payload: ReminderOccurrence {
            identity: "reminder/meeting/occurrence/0".into(),
            reminder_identity: "reminder/meeting".into(),
            event_identity: "event/cross-zone-meeting".into(),
            delivery_kind: "notification/local".into(),
        },
    };
    let decision = scheduled
        .decide(
            &TriggerObservation::Civil {
                now: wall(1_020),
                clock_change_observed: false,
            },
            false,
        )
        .unwrap();
    assert_eq!(
        decision,
        ScheduledOccurrenceDecision::Ready { lateness_ticks: 20 }
    );

    let mut adapter = RecordingAdapter::default();
    assert_eq!(
        deliver_ready_reminder(&scheduled, decision, None, &mut adapter),
        Err(ReminderDeliveryRefusal::MissingAuthority)
    );
    assert!(adapter.delivered.is_empty());

    let grant = AuthorityBinding {
        grant_id: AuthorityGrantId::from("grant/reminder/local"),
        contract_id: AuthorityContractId::from(REMINDER_DELIVERY_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(REMINDER_DELIVER_OPERATION),
        subject_kind: KindId::from(REMINDER_DELIVER_KIND),
        host_id: HostId::from("host/reminder"),
        boot_id: BootId::from("boot/reminder"),
        capability_id: CapabilityId::from("capability/reminder"),
    };
    let receipt = deliver_ready_reminder(&scheduled, decision, Some(&grant), &mut adapter).unwrap();
    assert_eq!(adapter.delivered, ["reminder/meeting/occurrence/0"]);
    assert_eq!(receipt.event_identity, "event/cross-zone-meeting");
    assert_eq!(scheduled.payload.event_identity, receipt.event_identity);
}

#[derive(Default)]
struct RecordingAdapter {
    delivered: Vec<String>,
}

impl HostedReminderAdapter for RecordingAdapter {
    fn deliver(&mut self, reminder: &ReminderOccurrence) -> Result<(), ReminderAdapterError> {
        self.delivered.push(reminder.identity.clone());
        Ok(())
    }
}

fn wall(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "unix/utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}
