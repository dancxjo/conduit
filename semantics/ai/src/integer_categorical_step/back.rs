use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoricalStepReceipt {
    pub model_content: [u8; 32],
    pub input: [u8; 32],
    pub output: [u8; 32],
    pub invocation: u64,
    pub work_units: u64,
}
pub struct CategoricalStepBack {
    profile: Arc<PreparedCategoricalStep>,
    input: IntegerCollectionCodec,
    output: IntegerCollectionCodec,
    indices: Vec<u64>,
    scores: Vec<i64>,
    flow: bool,
    committed: u64,
    staged: Option<CategoricalStepReceipt>,
    last: Option<CategoricalStepReceipt>,
    cancelled: bool,
    #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
    writer: Option<invocation::InvocationWriter>,
    #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
    reservation: Option<invocation::Reservation>,
}
impl CategoricalStepBack {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        profile: Arc<PreparedCategoricalStep>,
        flow: bool,
    ) -> Result<Self, String> {
        if PORTS < 1 || fuel < 2 {
            return Err("categorical step budget".into());
        }
        profile.verify_placement(gear, flow)?;
        let (_, outputs, lookups) = profile.dimensions();
        Ok(Self {
            input: IntegerCollectionCodec::prepare(&profile.indices, lookups)?,
            output: IntegerCollectionCodec::prepare(&profile.scores, outputs)?,
            indices: vec![0; lookups],
            scores: vec![0; outputs],
            profile,
            flow,
            committed: 0,
            staged: None,
            last: None,
            cancelled: false,
            #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
            writer: None,
            #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
            reservation: None,
        })
    }
    #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
    pub(super) fn observe(
        &mut self,
        outlet: &invocation::CategoricalInvocationOutlet,
        gear: &PlannedGear,
    ) -> Result<(), String> {
        self.writer = Some(outlet.writer(gear, &self.profile, self.flow)?);
        Ok(())
    }
    pub fn committed_invocations(&self) -> u64 {
        self.committed
    }
    pub fn last_receipt(&self) -> Option<CategoricalStepReceipt> {
        self.last
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for CategoricalStepBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = None;
        #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
        {
            self.reservation = None;
        }
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 3100,
            });
        }
        if !self.flow && self.committed != 0 {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(3101)
            };
        }
        if self.committed == u64::MAX {
            return fail(3102);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(3103);
        };
        #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
        if let Some(writer) = &self.writer {
            let Some(reservation) = writer.reserve() else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::StorageExhausted,
                    detail: 3106,
                });
            };
            self.reservation = Some(reservation);
        }
        if self.input.decode(bytes, &mut self.indices).is_err()
            || self
                .profile
                .model
                .infer_indices_into(&self.indices, &mut self.scores)
                .is_err()
        {
            return fail(3104);
        }
        let output = self.output.encode(&self.scores);
        let receipt = CategoricalStepReceipt {
            model_content: self.profile.model.identity(),
            input: semantic_digest("numeric/categorical-indices@1", bytes),
            output: semantic_digest("numeric/categorical-scores@1", output),
            invocation: self.committed + 1,
            work_units: self.profile.model.work_units(),
        };
        #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
        if let Some(reservation) = &mut self.reservation {
            reservation.stage(bytes, output, receipt.invocation);
        }
        if io.consume(PortId(0)).is_err()
            || io.send_prepared(PortId(0), output.len() as u32).is_err()
        {
            return fail(3105);
        }
        self.staged = Some(receipt);
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged.is_some()).then_some(self.output.encoded.as_slice())
    }
    fn step_committed(&mut self) {
        if let Some(receipt) = self.staged.take() {
            self.committed += 1;
            self.last = Some(receipt);
            #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
            if let Some(reservation) = self.reservation.take() {
                reservation.publish();
            }
        }
    }
    fn cancel(&mut self) {
        #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
        {
            self.reservation = None;
        }
        self.cancelled = true;
        self.staged = None;
    }
}
