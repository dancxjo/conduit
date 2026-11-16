//! Bounded alloc preparation for the shared window Step. No device effect.
//! Window state advances only after the wrapper observes actual Step commit.
use crate::{
    measurement_window_value_bounds, BoundedMeasurementWindow, MeasurementWindowRefusal,
    MEASUREMENT_WINDOW_FINALIZE,
};
use alloc::vec::Vec;
use conduit_kernel::{Failure, FailureCode, HostCallId, RequestId};

pub const MEASUREMENT_WINDOW_PROFILE_CALL: HostCallId = HostCallId(0);
pub const MEASUREMENT_WINDOW_SAMPLE_CALL: HostCallId = HostCallId(1);
pub struct PreparedMeasurementWindow {
    committed: Option<BoundedMeasurementWindow>,
    candidate: Option<BoundedMeasurementWindow>,
    profile_type: Vec<u8>,
    sample_type: Vec<u8>,
    window_type: Vec<u8>,
    input: Vec<u8>,
    output: Vec<u8>,
    cached: Option<(RequestId, HostCallId, Result<bool, Failure>)>,
    pending: bool,
    bounds: [u32; 3],
    cancelled: bool,
}
impl PreparedMeasurementWindow {
    pub fn new() -> Self {
        let b = measurement_window_value_bounds();
        Self {
            committed: None,
            candidate: None,
            profile_type: crate::measurement_window_profile_type()
                .canonical_bytes()
                .expect("profile Type"),
            sample_type: crate::measurement_sample_type()
                .canonical_bytes()
                .expect("sample Type"),
            window_type: crate::measurement_window_type()
                .canonical_bytes()
                .expect("window Type"),
            input: Vec::with_capacity(b[0].max(b[1]) as usize),
            output: Vec::with_capacity(b[2] as usize),
            cached: None,
            pending: false,
            bounds: b,
            cancelled: false,
        }
    }
    /// Initial owned buffers only. Dynamic window candidates remain covered by
    /// the separate whole-Play workspace reservation, never a hidden new pool.
    pub fn prepared_backing_bytes(&self) -> usize {
        self.profile_type.capacity()
            + self.sample_type.capacity()
            + self.window_type.capacity()
            + self.input.capacity()
            + self.output.capacity()
    }
    pub fn committed_window(&self) -> Option<&BoundedMeasurementWindow> {
        self.committed.as_ref()
    }
    /// Exact retries borrow the original cached result and never reapply a sample.
    pub fn execute(
        &mut self,
        request: RequestId,
        call: HostCallId,
        input: &[u8],
    ) -> Result<Option<&[u8]>, Failure> {
        if self.cancelled {
            return Err(failure(38));
        }
        if let Some((old, old_call, outcome)) = self.cached {
            if request == old {
                if call != old_call || input != self.input {
                    return Err(failure(36));
                }
                return outcome.map(|has_output| has_output.then_some(self.output.as_slice()));
            }
            if self.pending || old.0.checked_add(1) != Some(request.0) {
                return Err(failure(36));
            }
        } else if request != RequestId(0) {
            return Err(failure(36));
        }
        let b = self.bounds;
        let limit = if call == MEASUREMENT_WINDOW_PROFILE_CALL {
            b[0]
        } else if call == MEASUREMENT_WINDOW_SAMPLE_CALL {
            b[1]
        } else {
            return Err(failure(24));
        };
        if input.len() > limit as usize {
            return Err(failure(2));
        }
        // No input/cache publication precedes preparation of the complete next state.
        let prepared = self.prepare(call, input);
        self.input.clear();
        self.input.extend_from_slice(input);
        self.cached = Some((request, call, prepared));
        self.pending = true;
        prepared.map(|has_output| has_output.then_some(self.output.as_slice()))
    }
    fn prepare(&mut self, call: HostCallId, input: &[u8]) -> Result<bool, Failure> {
        if call == MEASUREMENT_WINDOW_PROFILE_CALL {
            if self.committed.is_some() {
                return Err(failure(1));
            }
            let bytes = payload(
                input,
                &self.profile_type,
                crate::MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES,
            )?;
            let profile =
                crate::decode_measurement_window_profile(bytes).map_err(|_| failure(3))?;
            // Existing authored MeasurementWindowProfile Type has Text<=128B.
            if profile.clock_basis.len() > 128 {
                return Err(failure(11));
            }
            self.candidate = Some(BoundedMeasurementWindow::new(profile).map_err(window_failure)?);
            return Ok(false);
        }
        let window = self.committed.as_ref().ok_or(failure(5))?;
        if input == MEASUREMENT_WINDOW_FINALIZE {
            let encoded = crate::encode_measurement_window(window).map_err(|_| failure(22))?;
            if self.window_type.len() + 5 + encoded.len() > self.bounds[2] as usize {
                return Err(failure(23));
            }
            self.output.clear();
            self.output.extend_from_slice(&self.window_type);
            self.output.push(0);
            self.output
                .extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            self.output.extend_from_slice(&encoded);
            return Ok(true);
        }
        let bytes = payload(
            input,
            &self.sample_type,
            crate::MAXIMUM_MEASUREMENT_SAMPLE_BYTES,
        )?;
        let sample = crate::decode_measurement_sample(bytes).map_err(|_| failure(7))?;
        // Clone allocates only under the admitted hosted workspace, never Step.
        // A Vec clone starts at len<=64; one push can reserve at most128 slots.
        let mut candidate = window.clone();
        candidate.push(sample).map_err(window_failure)?;
        self.candidate = Some(candidate);
        Ok(false)
    }
    /// Must be called ONLY from the Back's actual step_committed callback for
    /// that exact consumed completion. Moves owners without allocation.
    pub fn commit(&mut self, request: RequestId) -> Result<(), Failure> {
        let Some((expected, _, Ok(_))) = self.cached else {
            return Err(failure(37));
        };
        if !self.pending || expected != request {
            return Err(failure(37));
        }
        if let Some(window) = self.candidate.take() {
            self.committed = Some(window)
        }
        self.pending = false;
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.candidate = None;
        self.pending = false;
    }
}
impl Default for PreparedMeasurementWindow {
    fn default() -> Self {
        Self::new()
    }
}
fn payload<'a>(bytes: &'a [u8], ty: &[u8], maximum: usize) -> Result<&'a [u8], Failure> {
    let node = bytes.strip_prefix(ty).ok_or(failure(2))?;
    if node.len() < 5 || node[0] != 0 {
        return Err(failure(2));
    }
    let len = u32::from_le_bytes(node[1..5].try_into().map_err(|_| failure(2))?) as usize;
    if len > maximum || node.len() != 5 + len {
        return Err(failure(2));
    }
    Ok(&node[5..])
}
fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}
fn window_failure(error: MeasurementWindowRefusal) -> Failure {
    use MeasurementWindowRefusal::*;
    failure(match error {
        CapacityOutOfBounds => 10,
        InvalidClockProfile => 11,
        InvalidRange => 12,
        InvalidTimestamp => 13,
        UnitMismatch => 14,
        UncertaintyUnitMismatch => 15,
        NegativeUncertainty => 16,
        ClockMismatch => 17,
        TimestampRegression => 18,
        OutOfRange => 19,
        Full => 20,
        DiscardCountOverflow => 21,
        PointDifferenceRequired => 22,
    })
}
#[cfg(test)]
#[path = "measurement_window_provider/tests.rs"]
mod tests;
