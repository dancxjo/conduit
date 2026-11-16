//! Real observer/mailbox/scheduler protocol; Echo is a test Back, not a native
//! record service or evidence of physical sensing/transport/durable custody.
use super::*;
use std::vec::Vec;

struct CountExisting<'a>(&'a core::cell::Cell<u32>);
impl<K> Observer<K> for CountExisting<'_> {
    type Error = Infallible;
    fn before_step(&mut self, _: &mut K) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
    fn completed(&mut self, _: &mut K, _: Dispatch) -> Result<(), Self::Error> {
        Ok(())
    }
}
use conduit_kernel::{
    dictionary_live_sign_archive::DictionaryLiveSignLog, live_sign_archive::ArchiveLimits,
};

type Evidence = DictionaryLiveSignLog<128, 4, 48, 2, 2>;
fn evidence(events: u32) -> (crate::execution_sign_budget::DuplexSignBudget, Evidence) {
    let budget = crate::execution_sign_budget::DuplexSignBudget::select(55, 64, 24576).unwrap();
    let signs = Evidence::try_prepare_dictionary(
        budget.prepare().unwrap(),
        ArchiveLimits {
            retained_events: events,
            retained_bytes: 2097152,
            archive_backing_bytes: 8192,
            transfer_overlap_bytes: 12288,
        },
    )
    .unwrap();
    (budget, signs)
}

#[test]
fn archive_renews_only_after_real_delivery_and_handles_next_input_in_same_turn() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let (mut input, mut output) = ([0; 8], [0; 8]);
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let (budget, signs) = evidence(32768);
    let mut observer =
        ForeObserver::with_archived_duplex_evidence_window(Existing, observer, budget);
    let mut kernel = profile_kernel_with_signs::<0, 0, 0, 1, _>(8, signs);
    port.stage_ingress(0, &10u64.to_le_bytes()).unwrap();
    for sequence in 0..1264 {
        observer.before_step(&mut kernel).unwrap();
        delivered_input(&port, sequence);
        kernel.step().unwrap();
        observer.before_step(&mut kernel).unwrap();
        let event = Event::EgressOffered {
            sequence,
            length: 8,
        };
        assert_eq!(port.next_event().unwrap(), Some(event));
        port.with_egress(sequence, |bytes| {
            assert_eq!(bytes, &(sequence + 10).to_le_bytes())
        })
        .unwrap();
        port.sent(event).unwrap();
        port.peer_accepted(sequence).unwrap();
        observer.before_step(&mut kernel).unwrap();
        // Accepted output and empty IO are not delivered-result evidence.
        assert_eq!(
            observer.evidence_window_totals(),
            (sequence + 1, sequence, sequence)
        );
        if sequence < 1263 {
            port.stage_ingress(sequence + 1, &(sequence + 11).to_le_bytes())
                .unwrap();
        }
        port.peer_delivered(sequence).unwrap();
        let decisions = kernel.decisions();
        let invocations = kernel.logical_invocations();
        observer.before_step(&mut kernel).unwrap();
        assert_eq!(kernel.decisions(), decisions);
        assert_eq!(kernel.logical_invocations(), invocations);
        assert_eq!(
            observer.evidence_window_totals(),
            (
                if sequence < 1263 {
                    sequence + 2
                } else {
                    sequence + 1
                },
                sequence + 1,
                sequence + 1
            )
        );
    }
    assert_eq!(kernel.values().used_items(), 0);
    let remote: Vec<_> = kernel
        .signs()
        .events_with_remote()
        .filter_map(|entry| entry.remote)
        .collect();
    assert_eq!(remote.len(), 1264 * 4);
    for sequence in 0..1264 {
        assert_eq!(
            remote
                .iter()
                .filter(|identity| identity.sequence == sequence)
                .count(),
            4
        );
    }
    assert_eq!(observer.evidence_window_totals(), (1264, 1264, 1264));
}

#[test]
fn archive_refusal_keeps_pressure_and_evidence_without_stopping_observer() {
    let pins = Pins::new(c::COUNT_INFO_ID);
    let (mut input, mut output) = ([0; 8], [0; 8]);
    let bound = make_bound(&pins, &mut input, &mut output);
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let (budget, signs) = evidence(1);
    let ticks = core::cell::Cell::new(0);
    let mut observer =
        ForeObserver::with_archived_duplex_evidence_window(CountExisting(&ticks), observer, budget);
    let mut kernel = profile_kernel_with_signs::<0, 0, 0, 1, _>(8, signs);
    port.stage_ingress(0, &10u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    delivered_input(&port, 0);
    kernel.step().unwrap();
    observer.before_step(&mut kernel).unwrap();
    let event = Event::EgressOffered {
        sequence: 0,
        length: 8,
    };
    assert_eq!(port.next_event().unwrap(), Some(event));
    port.sent(event).unwrap();
    port.peer_accepted(0).unwrap();
    port.peer_delivered(0).unwrap();
    port.stage_ingress(1, &11u64.to_le_bytes()).unwrap();
    observer.before_step(&mut kernel).unwrap();
    assert_eq!(observer.evidence_window_totals(), (1, 1, 0));
    assert_eq!(port.next_event().unwrap(), Some(Event::IngressPressure(1)));
    let retained = kernel.signs().events_with_remote().collect::<Vec<_>>();
    let before = ticks.get();
    for _ in 0..3 {
        observer.before_step(&mut kernel).unwrap();
    }
    assert_eq!(
        kernel.signs().events_with_remote().collect::<Vec<_>>(),
        retained
    );
    assert_eq!(kernel.signs().archived_event_count(), 0);
    assert_eq!(observer.evidence_window_totals(), (1, 1, 0));
    assert_eq!(ticks.get(), before + 3);
}
