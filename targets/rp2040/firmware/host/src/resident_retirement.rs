//! Bounded cancellation/retirement of the existing production kernels.
use super::*;

/// Own terminal evidence before restoring providers. Resource release remains
/// an independent Root action and must refuse if any Back could not return.
pub struct RetiredKernel {
    pub signs: FixedSignLog<16>,
    pub providers_returned: bool,
}

impl BoundKernel<'_> {
    /// A cancellation refusal returns the same boxed kernel and its original
    /// providers. No allocation, resource release or implicit retry occurs.
    #[allow(
        clippy::result_large_err,
        reason = "failure retains the exact existing kernel ownership"
    )]
    pub fn cancel_and_retire(mut self) -> Result<RetiredKernel, (Self, SchedulerError)> {
        let cancelled = match &mut self {
            Self::One(kernel) => kernel.cancel(),
            Self::Two(kernel) => kernel.cancel(),
            Self::Three(kernel) => kernel.cancel(),
            Self::Four(kernel) => kernel.cancel(),
            Self::Five(kernel) => kernel.cancel(),
        };
        if let Err(error) = cancelled {
            return Err((self, error));
        }
        Ok(match self {
            Self::One(kernel) => retire(kernel),
            Self::Two(kernel) => retire(kernel),
            Self::Three(kernel) => retire(kernel),
            Self::Four(kernel) => retire(kernel),
            Self::Five(kernel) => retire(kernel),
        })
    }
}

fn retire<const N: usize, const C: usize, const V: usize>(
    kernel: Box<Kernel<'_, N, C, V>>,
) -> RetiredKernel {
    // No await or user operation intervenes after successful kernel cancel,
    // which synchronously clears queue, value and pending Host Call ownership.
    let retired = (*kernel)
        .try_retire()
        .unwrap_or_else(|_| unreachable!("successfully cancelled kernel must be drained"));
    let signs = retired.signs;
    let mut providers_returned = true;
    for back in retired.drivers {
        if back.finish_retired().is_err() {
            // Dropping a failed return leaves that exact inventory unavailable.
            // The caller retains evidence and must keep reservations fenced.
            providers_returned = false;
        }
    }
    RetiredKernel {
        signs,
        providers_returned,
    }
}
