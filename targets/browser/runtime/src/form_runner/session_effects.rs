//! Bounded pending platform effects of the ordinary browser session.
use super::{engine, TourProgress, TourSession};
impl TourSession {
    pub(super) fn advance(&mut self) -> Result<TourProgress, String> {
        if self.pending.len() != 1 {
            return Err("completion requires an exact effect identity".into());
        }
        self.complete_pending(0, None)
    }
    pub(super) fn advance_with_output(&mut self, output: &[u8]) -> Result<TourProgress, String> {
        if self.pending.len() != 1 {
            return Err("completion requires an exact effect identity".into());
        }
        self.complete_pending(0, Some(output))
    }
    pub(super) fn complete_effect(
        &mut self,
        play: &str,
        placement: &str,
        request: u32,
        output: Option<&[u8]>,
    ) -> Result<TourProgress, String> {
        if play != self.active_play_id.as_str() {
            return Err("stale browser Play completion".into());
        }
        let index = self
            .pending
            .iter()
            .position(|effect| {
                effect.request.request.0 == request
                    && super::placement_in_fragments(&self.fragments, effect.request.node)
                        .is_some_and(|(_, gear)| gear.placement_id.as_str() == placement)
            })
            .ok_or("unknown or completed browser effect identity")?;
        self.complete_pending(index, output)
    }
    fn complete_pending(
        &mut self,
        index: usize,
        output: Option<&[u8]>,
    ) -> Result<TourProgress, String> {
        let effect = self
            .pending
            .get(index)
            .ok_or("pending browser effect is absent")?;
        match output {
            Some(bytes) => {
                engine::complete_host_effect_with_output(&mut self.scheduler, effect, bytes)?
            }
            None => engine::complete_host_effect(&mut self.scheduler, effect)?,
        }
        match effect.effect {
            engine::BrowserHostEffect::Timer { .. } => self.timer_completions += 1,
            engine::BrowserHostEffect::Manifestation(_) => self.manifestation_completions += 1,
            _ => {}
        }
        self.pending.remove(index);
        self.poll_effect()
    }
    pub(super) fn poll_effect(&mut self) -> Result<TourProgress, String> {
        if let Some(cancellation) = self.poll_cancellation()? {
            return Ok(cancellation);
        }
        match engine::drive_with_placement(&mut self.scheduler, |node| {
            super::placement_in_fragments(&self.fragments, node).map(|(_, gear)| gear)
        })? {
            engine::DriveStatus::Effect(effect) => {
                if self.pending.len() == self.pending.capacity() {
                    return Err("browser pending effect capacity exhausted".into());
                }
                self.pending.push(effect);
                Ok(TourProgress::Effect(Box::new(
                    self.project_pending_effect(self.pending.len() - 1)?,
                )))
            }
            engine::DriveStatus::Waiting { pending_effects } => {
                if let Some(cancellation) = self.poll_cancellation()? {
                    return Ok(cancellation);
                }
                Ok(TourProgress::Waiting {
                    schema: "conduit.browser/pending-effects@1",
                    disposition: "waiting",
                    active_play_id: self.active_play_id.as_str().into(),
                    pending_effects,
                })
            }
            engine::DriveStatus::Complete if self.pending.is_empty() => {
                Ok(TourProgress::Receipt(Box::new(self.completed_receipt())))
            }
            engine::DriveStatus::Complete => {
                Err("kernel completed with outstanding platform effects".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlated_completion_preserves_other_pending_effects_and_rejects_stale_identity() {
        let source = "form concurrent {\n button: input/button(maximum-transitions = 1)\n state: input/button-indicator-state\n indicator: presentation/indicator-state\n clock: time/every(freq = 100ms)\n count: state/count(start = 0)\n show: presentation/count(maximum-values = 5)\n button > state > indicator\n clock.tick > count.bump\n count.value > show.value\n}\n";
        let (mut session, _) =
            TourSession::prepare("browser/test", "boot/test", source, 1).unwrap();
        let capacity = session.pending.capacity();
        for _ in 0..4 {
            if matches!(session.poll_effect().unwrap(), TourProgress::Waiting { .. }) {
                break;
            }
        }
        let button = session
            .pending
            .iter()
            .position(|effect| matches!(effect.effect, engine::BrowserHostEffect::ButtonTransition))
            .unwrap();
        let request = session.pending[button].request;
        let placement = session.fragments[0].placements[usize::from(request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let play = session.active_play_id.as_str().to_owned();
        let count = session.pending.len();
        assert!(count >= 2);
        assert!(session.advance().is_err());
        assert!(session
            .complete_effect("stale", &placement, request.request.0, None)
            .is_err());
        assert!(session
            .complete_effect(&play, "unknown", request.request.0, None)
            .is_err());
        assert_eq!(session.pending.len(), count);
        let bytes = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert!(matches!(
            session
                .complete_effect(&play, &placement, request.request.0, Some(&bytes))
                .unwrap(),
            TourProgress::Effect(_)
        ));
        assert!(session
            .complete_effect(&play, &placement, request.request.0, Some(&bytes))
            .is_err());
        assert!(session
            .pending
            .iter()
            .any(|effect| matches!(effect.effect, engine::BrowserHostEffect::Timer { .. })));
        assert_eq!(session.pending.capacity(), capacity);
        assert_eq!(session.cancel().unwrap().disposition, "cancelled");
    }
}

#[cfg(test)]
#[test]
fn pressed_attempt_observes_clock_before_requesting_its_deadline() {
    let source = "form timed {\n button: input/button(maximum-transitions = 5)\n attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 5, timeout-ms = 1000ms)\n derive: time/ordered-event-intervals\n button.transition > attempt.transition\n attempt.events > derive.events\n}\n";
    let (mut session, _) =
        TourSession::prepare("browser/clock-test", "boot/clock-test", source, 1).unwrap();
    let bytes = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut progress = session.advance_with_output(&bytes).unwrap();
    for _ in 0..3 {
        if matches!(&progress, TourProgress::Effect(effect) if matches!(**effect, super::TourHostEffect::ClockObservation(_)))
        {
            break;
        }
        progress = session.poll_effect().unwrap();
    }
    assert!(
        matches!(progress, TourProgress::Effect(effect) if matches!(*effect, super::TourHostEffect::ClockObservation(_)))
    );
    let clock = session
        .pending
        .iter()
        .find(|pending| matches!(pending.effect, engine::BrowserHostEffect::ClockObservation))
        .unwrap();
    assert!(engine::complete_host_effect(&mut session.scheduler, clock).is_err());
    let play = session.active_play_id.as_str().to_owned();
    let placement = session.fragments[0].placements[usize::from(clock.request.node.0)]
        .placement_id
        .as_str()
        .to_owned();
    let request = clock.request.request.0;
    assert!(session
        .complete_effect(&play, &placement, request, Some(&[0]))
        .is_err());
    let progress = session
        .complete_effect(&play, &placement, request, Some(&100_u64.to_le_bytes()))
        .unwrap();
    assert!(
        matches!(progress, TourProgress::Effect(effect) if matches!(*effect, super::TourHostEffect::Timer(_)))
    );
    let button = session
        .pending
        .iter()
        .find(|pending| matches!(pending.effect, engine::BrowserHostEffect::ButtonTransition))
        .unwrap();
    let placement = session.fragments[0].placements[usize::from(button.request.node.0)]
        .placement_id
        .as_str()
        .to_owned();
    let request = button.request.request.0;
    let released = conduit_semantic_catalog::button_transition_value("button/primary", false, 1)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut progress = session
        .complete_effect(&play, &placement, request, Some(&released))
        .unwrap();
    for _ in 0..3 {
        if matches!(progress, TourProgress::Cancellation { .. }) {
            break;
        }
        progress = session.poll_effect().unwrap();
    }
    let TourProgress::Cancellation {
        placement_id,
        request_sequence,
        ..
    } = progress
    else {
        panic!("deadline must be cancelled before observing queued transition");
    };
    assert!(session
        .acknowledge_cancellation("stale", &placement_id, request_sequence)
        .is_err());
    let progress = session
        .acknowledge_cancellation(&play, &placement_id, request_sequence)
        .unwrap();
    assert!(
        matches!(progress, TourProgress::Effect(effect) if matches!(*effect, super::TourHostEffect::ClockObservation(_)))
    );
    assert!(session
        .acknowledge_cancellation(&play, &placement_id, request_sequence)
        .is_err());
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}
