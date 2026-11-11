//! Original Ready owners under independently verified installed permission.
//! This creates descriptive grants only; fresh scope/full admission/Start remain required.
use super::*;
use crate::continuation_native::ContinuationNativeContext;
impl<const W: usize> Issuers<W> {
    #[allow(clippy::too_many_arguments)]
    pub fn authorize_continuation<
        P: PulseReadProvider,
        T: OpenDrain,
        C: RendererClock,
        M: MonotonicWaitProvider,
    >(
        &mut self,
        context: ContinuationNativeContext<'_>,
        policy: Policy<'_>,
        pulse: &ReadyPulseBase<P>,
        write: &ReadyWriteBase<T>,
        renderer: &ReadyRenderer<C>,
        waits: &[Option<ReadyMonotonicWaitBase<M>>; W],
        operations: &Operations<'_, W>,
        remaining: impl Fn() -> usize,
    ) -> Result<(), Refusal> {
        self.pulse.authorize_continuation(
            context,
            policy,
            &NativeFacts::pulse(pulse),
            &operations.pulse,
            "mcu/original-pulse",
            remaining(),
        )?;
        self.write.authorize_continuation(
            context,
            policy,
            &NativeFacts::write(write),
            &operations.write,
            "mcu/original-write",
            remaining(),
        )?;
        self.renderer.authorize_continuation(
            context,
            policy,
            &NativeFacts::renderer(renderer),
            &operations.renderer,
            "mcu/original-renderer",
            remaining(),
        )?;
        for (slot, (owner, operation)) in waits.iter().zip(&operations.waits).enumerate() {
            match (owner, operation) {
                (Some(owner), Some(operation)) => {
                    self.waits[slot].authorize_continuation(
                        context,
                        policy,
                        &NativeFacts::wait(owner),
                        operation,
                        owner.native_identity().provider_instance_id.as_str(),
                        remaining(),
                    )?;
                }
                (None, None) => {}
                _ => return Err(Refusal::Scope),
            }
        }
        Ok(())
    }
}
