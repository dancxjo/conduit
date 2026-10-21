//! Guest-private owner of the existing canonical scheduler and storage.
use crate::{
    Scheduler, VALUE_BYTES,
    wire::{Request, Response},
};
use conduit_kernel::{SignQuery, SignRetentionGap, scheduler::SchedulerError};
pub struct Runtime {
    scheduler: Option<Scheduler>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self { scheduler: None }
    }
}
impl Runtime {
    pub fn gap(&self) -> Option<SignRetentionGap> {
        self.scheduler
            .as_ref()
            .and_then(|s| SignQuery::retention_gap(s.signs()))
    }
    pub fn execute(
        &mut self,
        request: Request,
        transfer: &mut [u8],
    ) -> Result<Response, SchedulerError> {
        if let Request::Init { bytes } = request {
            if self.scheduler.is_some() {
                return Err(SchedulerError::InvalidPlan);
            }
            let data = transfer
                .get(..bytes as usize)
                .ok_or(SchedulerError::InvalidPlan)?;
            let admission =
                crate::wire::admission::decode(data).map_err(|_| SchedulerError::InvalidPlan)?;
            self.scheduler = Some(admission.build()?);
            return Ok(Response::Ready);
        }
        let s = self.scheduler.as_mut().ok_or(SchedulerError::InvalidPlan)?;
        match request {
            Request::Step => s.step().map(Response::Status),
            Request::Next => {
                let request = s.next_host_request();
                transfer.fill(0);
                if let Some(request) = request {
                    let input = s.host_value(request.input.value)?;
                    if input.len() > VALUE_BYTES {
                        return Err(SchedulerError::HostCallOutputExceeded);
                    }
                    transfer
                        .get_mut(..input.len())
                        .ok_or(SchedulerError::HostCallOutputExceeded)?
                        .copy_from_slice(input);
                }
                Ok(Response::Next(request))
            }
            Request::Store { bytes } => {
                if bytes as usize > VALUE_BYTES {
                    return Err(SchedulerError::HostCallOutputExceeded);
                }
                s.store_host_value(
                    transfer
                        .get(..bytes as usize)
                        .ok_or(SchedulerError::HostCallOutputExceeded)?,
                )
                .map(Response::Stored)
            }
            Request::Complete {
                node,
                request,
                outcome,
            } => {
                s.complete_host_call(node, request, outcome)?;
                Ok(Response::Ack)
            }
            Request::Cancel => {
                s.cancel()?;
                Ok(Response::Cancelled)
            }
            Request::Init { .. } => Err(SchedulerError::InvalidPlan),
        }
    }
}
