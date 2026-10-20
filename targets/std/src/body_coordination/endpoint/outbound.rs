//! Outbound session state and retained kernel-offer ownership.
use super::{
    CoordinationEndpoint, CoordinationOffer, RemoteCordDirection, SchedulerStatus, SessionBinding,
    SessionMachine,
};

impl CoordinationEndpoint {
    // Borrow immutable binding and mutable protocol state independently. This
    // does not change the selected connection or admit an outbound transition.
    pub(in crate::body_coordination) fn outbound_session(
        &mut self,
        direction: RemoteCordDirection,
    ) -> (&SessionBinding, &mut SessionMachine) {
        let session = self.endpoint_session_mut(direction);
        (&session.binding, &mut session.machine)
    }

    pub fn next_offer(&mut self) -> Result<CoordinationOffer, String> {
        loop {
            self.complete_face_request()?;
            let (endpoint, cord) = self.remote(RemoteCordDirection::Egress);
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(|error| format!("{error:?}"))?
            {
                return Ok(CoordinationOffer {
                    sequence: offer.sequence,
                    bytes: self
                        .scheduler
                        .host_value(offer.value)
                        .map_err(|error| format!("{error:?}"))?
                        .to_vec(),
                });
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("coordination endpoint idle before offer".into())
                }
                SchedulerStatus::Drained => {
                    return Err("coordination endpoint completed before offer".into())
                }
                SchedulerStatus::Cancelled => return Err("coordination endpoint cancelled".into()),
            }
        }
    }
}
