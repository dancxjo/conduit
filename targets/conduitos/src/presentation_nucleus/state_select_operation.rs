//! Fixed-storage Backs for the portable current Scalar selector.

use conduit_core::{BOOL_ENCODED_LEN, InfoBool, SCALAR_ENCODED_LEN, Scalar};
use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{CanonicalValue, PortId, ValueRef};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(super) enum StateSelectBack {
    Source {
        values: [Option<ValueRef>; 2],
        phase: u8,
    },
    Select {
        selector: Option<bool>,
        candidates: [Option<[u8; SCALAR_ENCODED_LEN]>; 2],
        closed: [bool; 3],
        input_cursor: usize,
    },
    Sink {
        pending: bool,
        next_request: u32,
    },
}

#[cfg(test)]
impl StateSelectBack {
    pub(super) fn accept_for_test(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> Result<Option<[u8; SCALAR_ENCODED_LEN]>, conduit_kernel::Failure> {
        let Self::Select {
            selector,
            candidates,
            closed,
            ..
        } = self
        else {
            return Err(failure(72));
        };
        accept_select(selector, candidates, closed, port, value, canonical)
    }
}

impl StepOperation<PORTS> for StateSelectBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { values, phase } => step_source(values, phase, io),
            Self::Select {
                selector,
                candidates,
                closed,
                input_cursor,
            } => step_select(selector, candidates, closed, input_cursor, io, input_bytes),
            Self::Sink {
                pending,
                next_request,
            } => step_sink(pending, next_request, io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Select {
                selector,
                candidates,
                ..
            } => {
                *selector = None;
                *candidates = [None; 2];
            }
            Self::Sink { pending, .. } => *pending = false,
            _ => {}
        }
    }
}

fn step_source(
    values: &[Option<ValueRef>; 2],
    phase: &mut u8,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *phase == 1 {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if request != conduit_kernel::RequestId(0)
            || outcome.disposition != conduit_kernel::HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(71);
        }
    }
    let index = usize::from(*phase == 1);
    let Some(value) = values[index] else {
        return StepOutcome::Complete;
    };
    if !io.output_ready(PortId(0)) {
        return StepOutcome::Await;
    }
    if io.send(PortId(0), value).is_err() {
        return invalid(71);
    }
    if index == 1 || values[1].is_none() {
        *phase = 2;
        return StepOutcome::Complete;
    }
    if io
        .request_host_call(
            conduit_kernel::RequestId(0),
            conduit_kernel::HostCallId(0),
            conduit_kernel::BoundedValueRef::new(value, SCALAR_ENCODED_LEN as u32)
                .expect("source values fit the Scalar upper bound"),
        )
        .is_err()
    {
        return invalid(71);
    }
    *phase = 1;
    StepOutcome::Progress
}

fn step_select(
    selector: &mut Option<bool>,
    candidates: &mut [Option<[u8; SCALAR_ENCODED_LEN]>; 2],
    closed: &mut [bool; 3],
    input_cursor: &mut usize,
    io: &mut StepIo<PORTS>,
    input_bytes: &StepInputBytes<'_, PORTS>,
) -> StepOutcome {
    for offset in 0..3 {
        let index = (*input_cursor + offset) % 3;
        let port = PortId(index as u16);
        if let Some(value) = io.input(port) {
            let Some(canonical) = input_bytes.input(port) else {
                return invalid(72);
            };
            let would_emit = match port {
                PortId(0) => candidates.iter().all(Option::is_some),
                PortId(1) => selector.is_some() && candidates[1].is_some(),
                PortId(2) => selector.is_some() && candidates[0].is_some(),
                _ => false,
            };
            if would_emit && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let selected = match accept_select(selector, candidates, closed, port, value, canonical)
            {
                Ok(selected) => selected,
                Err(failure) => return StepOutcome::Fail(failure),
            };
            if io.consume(port).is_err() {
                return invalid(72);
            }
            *input_cursor = (index + 1) % 3;
            if let Some(value) = selected
                && io
                    .send_canonical(
                        PortId(0),
                        CanonicalValue::new(&value).expect("Scalar fits the canonical value bound"),
                    )
                    .is_err()
            {
                return invalid(72);
            }
            return StepOutcome::Progress;
        }
    }
    for (index, is_closed) in closed.iter_mut().enumerate() {
        let port = PortId(index as u16);
        if !*is_closed && io.input_closed(port) {
            if io.consume_closed(port).is_err() {
                return invalid(71);
            }
            *is_closed = true;
            return if closed.iter().all(|closed| *closed) {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
    }
    StepOutcome::Await
}

fn accept_select(
    selector: &mut Option<bool>,
    candidates: &mut [Option<[u8; SCALAR_ENCODED_LEN]>; 2],
    closed: &[bool; 3],
    port: PortId,
    value: ValueRef,
    canonical: &[u8],
) -> Result<Option<[u8; SCALAR_ENCODED_LEN]>, conduit_kernel::Failure> {
    match port {
        PortId(0) if value.byte_len == BOOL_ENCODED_LEN as u32 && !closed[0] => {
            let decoded = InfoBool::decode(canonical).map_err(|_| failure(72))?;
            *selector = Some(decoded.get());
        }
        PortId(1) | PortId(2)
            if value.byte_len == SCALAR_ENCODED_LEN as u32 && !closed[usize::from(port.0)] =>
        {
            Scalar::decode(canonical).map_err(|_| failure(72))?;
            candidates[usize::from(port.0 - 1)] =
                Some(canonical.try_into().map_err(|_| failure(72))?);
        }
        _ => return Err(failure(72)),
    }
    Ok(selector
        .and_then(|selected| candidates[usize::from(selected)])
        .filter(|_| candidates.iter().all(Option::is_some)))
}

fn step_sink(pending: &mut bool, next_request: &mut u32, io: &mut StepIo<PORTS>) -> StepOutcome {
    if *pending {
        let Some((_request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if outcome.disposition != conduit_kernel::HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(71);
        }
        *pending = false;
        return StepOutcome::Progress;
    }
    if let Some(value) = io.input(PortId(0)) {
        if value.byte_len != SCALAR_ENCODED_LEN as u32 {
            return invalid(71);
        }
        let request = conduit_kernel::RequestId(*next_request);
        if io.consume(PortId(0)).is_err()
            || io
                .request_host_call(
                    request,
                    conduit_kernel::HostCallId(0),
                    conduit_kernel::BoundedValueRef::new(value, SCALAR_ENCODED_LEN as u32)
                        .expect("exact Scalar length is within the sink bound"),
                )
                .is_err()
        {
            return invalid(71);
        }
        *next_request = next_request.saturating_add(1);
        *pending = true;
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(71);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(failure(detail))
}

fn failure(detail: u16) -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    }
}
