//! Finite canonical frame ownership for the transaction registry.
//! Frames are prepared before publication and move once into full receipts.
use super::*;
use crate::parser_session_canonical_ingress::{
    ParserCanonicalIngressRefusal, ParserCanonicalSourceExecutor,
    PreparedCanonicalParserSessionPort, PreparedParserExecutionFrames,
};
use conduit_plot::rust_binding::PreparedNativeRustBinding;

struct CanonicalPort<I, O, E> {
    port: RefCell<PreparedCanonicalParserSessionPort<I, O, E>>,
    frames: RefCell<Vec<PreparedParserExecutionFrames>>,
}
impl<
        I: PreparedNativeRustBinding,
        O: PreparedNativeRustBinding,
        E: ParserCanonicalSourceExecutor,
    > CancelIngress for Rc<CanonicalPort<I, O, E>>
{
    fn cancel(&self) {
        self.port.borrow_mut().cancel();
    }
}
pub(crate) struct GuardedCanonicalIngress<I, O, E> {
    owned: Rc<CanonicalPort<I, O, E>>,
    registry: Rc<TransactionIngressRegistry>,
}
pub(crate) enum CanonicalRegistryRefusal<E> {
    Stage(crate::parser_session_transaction::SessionTransactionRefusal),
    FramePressure,
    Execution(ParserCanonicalIngressRefusal<E>),
}
impl TransactionIngressRegistry {
    /// The pool's complete requested bytes must fit before its first allocation.
    /// Actual retained capacities are checked before any ingress is exposed.
    /// This receipt excludes Native conversion, evaluator and target ownership.
    pub(crate) fn register_canonical<
        I: PreparedNativeRustBinding + 'static,
        O: PreparedNativeRustBinding + 'static,
        E: ParserCanonicalSourceExecutor + 'static,
    >(
        self: &Rc<Self>,
        port: PreparedCanonicalParserSessionPort<I, O, E>,
        count: u32,
        input_bytes: usize,
        output_bytes: usize,
        maximum_frame_bytes: usize,
    ) -> Result<(GuardedCanonicalIngress<I, O, E>, usize), IngressRegistryRefusal> {
        if self.closed() {
            return Err(IngressRegistryRefusal::Cancelled);
        }
        if self.sealed.get() {
            return Err(IngressRegistryRefusal::Sealed);
        }
        if self.ports.borrow().len() == usize::from(self.limit) {
            return Err(IngressRegistryRefusal::Pressure);
        }
        let count = usize::try_from(count).map_err(|_| IngressRegistryRefusal::Limits)?;
        let header_bytes = count
            .checked_mul(core::mem::size_of::<PreparedParserExecutionFrames>())
            .ok_or(IngressRegistryRefusal::Limits)?;
        let requested = input_bytes
            .checked_add(output_bytes)
            .and_then(|bytes| bytes.checked_mul(count))
            .and_then(|bytes| bytes.checked_add(header_bytes))
            .ok_or(IngressRegistryRefusal::Limits)?;
        if count == 0 || requested > maximum_frame_bytes {
            return Err(IngressRegistryRefusal::Pressure);
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(count)
            .map_err(|_| IngressRegistryRefusal::Allocation)?;
        let mut retained = frames
            .capacity()
            .checked_mul(core::mem::size_of::<PreparedParserExecutionFrames>())
            .ok_or(IngressRegistryRefusal::Limits)?;
        for _ in 0..count {
            let frame = PreparedParserExecutionFrames::prepare(input_bytes, output_bytes)
                .map_err(|_| IngressRegistryRefusal::Allocation)?;
            retained = retained
                .checked_add(frame.retained_capacity_bytes())
                .ok_or(IngressRegistryRefusal::Limits)?;
            if retained > maximum_frame_bytes {
                return Err(IngressRegistryRefusal::Pressure);
            }
            frames.push(frame);
        }
        let owned = Rc::new(CanonicalPort {
            port: RefCell::new(port),
            frames: RefCell::new(frames),
        });
        self.ports.borrow_mut().push(Box::new(owned.clone()));
        Ok((
            GuardedCanonicalIngress {
                owned,
                registry: self.clone(),
            },
            retained,
        ))
    }
}
impl<
        I: PreparedNativeRustBinding,
        O: PreparedNativeRustBinding,
        E: ParserCanonicalSourceExecutor,
    > GuardedCanonicalIngress<I, O, E>
{
    pub(crate) fn transact(
        &mut self,
        stage: &mut SessionTransactionStage<'_>,
        input: &[u8],
    ) -> Result<
        crate::parser_session_execution::ParserSessionExecution<I, O>,
        CanonicalRegistryRefusal<E::Error>,
    > {
        stage
            .before_ingress(&self.registry)
            .map_err(CanonicalRegistryRefusal::Stage)?;
        let frame = self.owned.frames.borrow_mut().pop();
        let Some(frame) = frame else {
            // A refusal after an earlier target consumption poisons this whole
            // stage; callers cannot ignore pool pressure and publish a prefix.
            stage.record_ingress(false, true);
            return Err(CanonicalRegistryRefusal::FramePressure);
        };
        // Arm whole-registry abandonment before crossing the target boundary.
        // A panic cannot skip the stage's cancellation by skipping a response.
        stage.record_ingress(true, false);
        let result = { self.owned.port.borrow_mut().execute(input, frame) };
        // Release both port and frame-pool borrows before whole-registry closure.
        // Conservative refusal closes siblings even if validation stopped early.
        stage.record_ingress(true, result.is_err());
        result.map_err(CanonicalRegistryRefusal::Execution)
    }
}
