//! Guest-private owner of unchanged canonical scheduler specializations/storage.
use crate::{
    Scheduler, VALUE_BYTES, morse, text,
    wire::{Request, Response, Stats},
};
use conduit_kernel::{SignQuery, SignRetentionGap, SignSink, scheduler::SchedulerError};
// The guest owns one fixed-capacity scheduler in its admitted state region.
// Keep every specialization inline: this no-allocator runtime cannot box it.
#[allow(clippy::large_enum_variant)]
enum Owner {
    Empty,
    Native(Scheduler),
    Text(text::Scheduler),
    Morse(morse::Scheduler),
}
pub struct Runtime {
    owner: Owner,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            owner: Owner::Empty,
        }
    }
}
// Each branch calls the same canonical FixedScheduler implementation. The text
// specialization retains its original capacities and Back lifecycle semantics.
macro_rules! scheduler {
    ($owner:expr, $s:ident, $body:expr) => {
        match $owner {
            Owner::Native($s) => $body,
            Owner::Text($s) => $body,
            Owner::Morse($s) => $body,
            Owner::Empty => return Err(SchedulerError::InvalidPlan),
        }
    };
}
impl Runtime {
    pub fn gap(&self) -> Option<SignRetentionGap> {
        match &self.owner {
            Owner::Empty => None,
            Owner::Native(s) => SignQuery::retention_gap(s.signs()),
            Owner::Text(s) => SignQuery::retention_gap(s.signs()),
            Owner::Morse(s) => SignQuery::retention_gap(s.signs()),
        }
    }
    pub fn execute(
        &mut self,
        request: Request,
        transfer: &mut [u8],
    ) -> Result<Response, SchedulerError> {
        match request {
            Request::Init { bytes }
            | Request::InitText { bytes }
            | Request::InitMorse { bytes } => {
                if !matches!(self.owner, Owner::Empty) {
                    return Err(SchedulerError::InvalidPlan);
                }
                let data = transfer
                    .get(..bytes as usize)
                    .ok_or(SchedulerError::InvalidPlan)?;
                self.owner = if matches!(request, Request::InitMorse { .. }) {
                    let admission = crate::wire::morse_admission::decode(data)
                        .map_err(|_| SchedulerError::InvalidPlan)?;
                    Owner::Morse(admission.build()?)
                } else if matches!(request, Request::InitText { .. }) {
                    let admission = crate::wire::text_admission::decode(data)
                        .map_err(|_| SchedulerError::InvalidPlan)?;
                    Owner::Text(admission.build()?)
                } else {
                    let admission = crate::wire::admission::decode(data)
                        .map_err(|_| SchedulerError::InvalidPlan)?;
                    Owner::Native(admission.build()?)
                };
                return Ok(Response::Ready);
            }
            _ => {}
        }
        let maximum = if matches!(self.owner, Owner::Morse(_)) {
            morse::MAX_VALUE_BYTES
        } else if matches!(self.owner, Owner::Text(_)) {
            text::MAX_VALUE_BYTES
        } else {
            VALUE_BYTES
        };
        scheduler!(
            &mut self.owner,
            s,
            match request {
                Request::Step => s.step().map(Response::Status),
                Request::Stats => Ok(Response::Stats(Stats {
                    decisions: s.decisions(),
                    signs: s.signs().len(),
                    pending: s.pending_host_call_count() as u16,
                })),
                Request::Next => {
                    let request = s.next_host_request();
                    transfer.fill(0);
                    if let Some(request) = request {
                        let input = s.host_value(request.input.value)?;
                        if input.len() > maximum {
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
                    if bytes as usize > maximum {
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
                Request::Init { .. } | Request::InitText { .. } | Request::InitMorse { .. } =>
                    Err(SchedulerError::InvalidPlan),
            }
        )
    }
}
