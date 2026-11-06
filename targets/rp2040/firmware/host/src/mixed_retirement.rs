//! Terminal Kernel evidence precedes native revocation and resource release.
use super::*;

pub struct RetiredMixedKernel<P> {
    pub signs: FixedSignLog<SIGNS>,
    pub residents_returned: bool,
    pub native_node: NodeId,
    pub call: I2cHostCall<P>,
}

impl<P: I2cProvider> BoundMixedKernel<'_, P> {
    /// Cancellation pressure returns the original kernel and both providers.
    /// Success drains the production kernel and returns resident loans. Root
    /// retains the native owner: it must explicitly revoke and retire that owner
    /// before releasing its reservation. No bus reset, Ready assertion, resource
    /// release, transport acknowledgment or new Play occurs here.
    #[allow(
        clippy::result_large_err,
        reason = "failure retains exact existing ownership"
    )]
    pub fn cancel_and_retire(mut self) -> Result<RetiredMixedKernel<P>, (Self, SchedulerError)> {
        if let Err(error) = self.kernel.cancel() {
            return Err((self, error));
        }
        let retired = (*self.kernel)
            .try_retire()
            .unwrap_or_else(|_| unreachable!("successfully cancelled kernel must be drained"));
        let mut residents_returned = true;
        let mut native_node = None;
        for (node, back) in retired.drivers.into_iter().enumerate() {
            match back {
                MixedBack::Resident(back) => {
                    if back.finish_retired().is_err() {
                        residents_returned = false;
                    }
                }
                MixedBack::Native(_) => native_node = Some(NodeId(node as u16)),
            }
        }
        Ok(RetiredMixedKernel {
            signs: retired.signs,
            residents_returned,
            native_node: native_node.expect("checked mixed profile has exactly one native driver"),
            call: self.call,
        })
    }
}
