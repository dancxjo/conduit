//! Exact kernel-requested cancellation, held until platform acknowledgement.
use super::{TourProgress, TourSession};
impl TourSession {
    pub(super) fn poll_cancellation(&mut self) -> Result<Option<TourProgress>, String> {
        if self.cancellation.is_none() {
            self.cancellation = self.scheduler.next_host_cancellation();
        }
        let Some(cancellation) = &self.cancellation else {
            return Ok(None);
        };
        let (_, placement) = super::placement_in_fragments(&self.fragments, cancellation.node)
            .ok_or("cancellation placement is absent")?;
        Ok(Some(TourProgress::Cancellation {
            schema: "conduit.browser/cancel-effect@1",
            effect_kind: "cancel",
            active_play_id: self.active_play_id.as_str().into(),
            placement_id: placement.placement_id.as_str().into(),
            request_sequence: cancellation.request.0,
        }))
    }
    pub(super) fn acknowledge_cancellation(
        &mut self,
        play: &str,
        placement: &str,
        request: u32,
    ) -> Result<TourProgress, String> {
        if play != self.active_play_id.as_str() {
            return Err("stale cancellation Play".into());
        }
        let cancellation = self
            .cancellation
            .as_ref()
            .ok_or("no kernel cancellation awaits acknowledgement")?;
        if cancellation.request.0 != request
            || !super::placement_in_fragments(&self.fragments, cancellation.node)
                .is_some_and(|(_, gear)| gear.placement_id.as_str() == placement)
        {
            return Err("cancellation identity differs from kernel request".into());
        }
        let index = self
            .pending
            .iter()
            .position(|effect| {
                effect.request.node == cancellation.node
                    && effect.request.request == cancellation.request
                    && effect.request.operation == cancellation.operation
            })
            .ok_or("cancelled effect is not pending")?;
        self.scheduler
            .complete_host_operation(
                cancellation.node,
                cancellation.request,
                conduit_kernel::HostOperationOutcome {
                    disposition: conduit_kernel::HostOperationDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| format!("{error:?}"))?;
        self.pending.remove(index);
        self.cancellation = None;
        self.poll_effect()
    }
}
