//! Exact pending platform effect descriptions from the active Plan.
use super::*;

impl TourSession {
    pub(super) fn project_pending_effect(
        &mut self,
        index: usize,
    ) -> Result<TourHostEffect, String> {
        let pending = self
            .pending
            .get(index)
            .ok_or("pending browser effect is absent")?;
        let (fragment, placement) =
            placement_in_fragments(&self.fragments, pending.request.node)
                .ok_or_else(|| "Host effect has no planned placement".to_string())?;
        match &pending.effect {
            engine::BrowserHostEffect::AudioCue => Ok(TourHostEffect::AudioCue(Box::new(
                super::audio::describe(self, placement, pending.request.request.0),
            ))),
            engine::BrowserHostEffect::Snapshot { .. } => {
                let request = engine::resource_effect::describe(&self.scheduler, pending)?;
                Ok(TourHostEffect::Snapshot(Box::new(
                    protocol::SnapshotEffect {
                        schema: "conduit.browser/resource-effect@1",
                        effect_kind: request.effect_kind,
                        active_play_id: self.active_play_id.as_str().into(),
                        placement_id: placement.placement_id.as_str().into(),
                        host_id: self.host_id.as_str().into(),
                        boot_id: self.boot_id.as_str().into(),
                        request_sequence: pending.request.request.0,
                        key: request.key.into(),
                        record: request.record.map(<[u8]>::to_vec),
                        source_interaction: self.source_interaction.clone(),
                    },
                )))
            }
            engine::BrowserHostEffect::Timer { duration_millis } => {
                Ok(TourHostEffect::Timer(Box::new(TourTimerEffect {
                    schema: "conduit.tour/timer-effect@1",
                    effect_kind: "timer",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    duration_millis: *duration_millis,
                    source_interaction: self.source_interaction.clone(),
                })))
            }
            engine::BrowserHostEffect::PointerEvent => {
                Ok(TourHostEffect::PointerEvent(Box::new(TourKeyEventEffect {
                    schema: "conduit.browser/pointer-event-effect@1",
                    effect_kind: "pointer-event",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
                        as u32,
                    source_interaction: self.source_interaction.clone(),
                })))
            }
            engine::BrowserHostEffect::ClockObservation => Ok(TourHostEffect::ClockObservation(
                Box::new(TourKeyEventEffect {
                    schema: "conduit.browser/clock-observation-effect@1",
                    effect_kind: "clock-observation",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: 8,
                    source_interaction: self.source_interaction.clone(),
                }),
            )),
            engine::BrowserHostEffect::KeyEvent => {
                Ok(TourHostEffect::KeyEvent(Box::new(TourKeyEventEffect {
                    schema: "conduit.tour/key-event-effect@1",
                    effect_kind: "key-event",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
                    source_interaction: self.source_interaction.clone(),
                })))
            }
            engine::BrowserHostEffect::ButtonTransition => Ok(TourHostEffect::ButtonTransition(
                Box::new(TourButtonTransitionEffect {
                    schema: "conduit.tour/button-transition-effect@1",
                    effect_kind: "button-transition",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
                    source_interaction: self.source_interaction.clone(),
                }),
            )),
            engine::BrowserHostEffect::Manifestation(manifestation) => {
                let partition = self
                    .fragments
                    .iter()
                    .position(|candidate| {
                        candidate.plan_id == fragment.plan_id
                            && candidate.fragment_id == fragment.fragment_id
                    })
                    .ok_or("manifestation partition is absent")?;
                let observation_sequence = pending.request.request.0;
                let presentation = bind_presentation(
                    &self.active_play_id,
                    &placement.placement_id,
                    u64::from(observation_sequence),
                );
                let (unit_millis, segments, text) = decode_manifestation(manifestation)?;
                let effect = TourEffect {
                    schema: "conduit.tour/manifestation-effect@3",
                    effect_kind: "manifestation",
                    source_document_id: fragment.source_document_id.as_str().into(),
                    checked_form_id: fragment.checked_form_id.as_str().into(),
                    expanded_form_id: fragment.expanded_form_id.as_str().into(),
                    plan_id: fragment.plan_id.as_str().into(),
                    fragment_id: fragment.fragment_id.as_str().into(),
                    active_play_id: self.active_play_id.as_str().into(),
                    presentation_id: presentation.presentation_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    presentation_kind: manifestation.kind_id.into(),
                    observation_sequence,
                    realization: self.realization.as_str(),
                    expanded_gears: self
                        .expanded_gears
                        .get(partition)
                        .ok_or("partition Gear evidence is absent")?
                        .clone(),
                    realization_backs: self
                        .realization_backs
                        .get(partition)
                        .ok_or("partition Back evidence is absent")?
                        .clone(),
                    unit_millis,
                    segments,
                    text,
                    source_interaction: self.source_interaction.clone(),
                };
                self.latest_presentation = Some(presentation);
                Ok(TourHostEffect::Manifestation(Box::new(effect)))
            }
        }
    }
}
