//! Actual allocation refusal observations, including pre-unwind backing custody.
use super::*;
#[test]
fn exact_kernel_and_sign_allocation_failures_are_observed_before_unwind() {
    for phase in [
        n::NetworkKernelAllocationPhase::Kernel,
        n::NetworkKernelAllocationPhase::Signs,
    ] {
        let kernel = core::mem::size_of::<n::PreparedNetworkKernel>();
        let (bytes, alignment) = match phase {
            n::NetworkKernelAllocationPhase::Kernel => {
                (kernel, core::mem::align_of::<n::PreparedNetworkKernel>())
            }
            n::NetworkKernelAllocationPhase::Signs => (
                n::CONTROL_SIGN_BACKING_BYTES,
                k::HostedSignLog::canonical_local_storage_alignment(),
            ),
        };
        let captured = core::cell::Cell::new(None);
        let held = core::cell::Cell::new(None);
        let refusal = allocation_probe::refuse_next_allocation_size(bytes);
        let (result, memory) = allocation_probe::observe(|| {
            n::PreparedNetworkKernelStorage::try_new_observing_failure(|failure| {
                captured.set(Some(failure));
                held.set(allocation_probe::current_observation().map(|m| m.retained_bytes()));
            })
        });
        drop(refusal);
        assert!(matches!(result, Err(n::ControlKernelRefusal::Storage)));
        assert_eq!(
            captured.get(),
            Some(n::NetworkKernelAllocationFailure {
                phase,
                requested_bytes: bytes,
                alignment
            })
        );
        assert_eq!(
            held.get(),
            Some(if phase == n::NetworkKernelAllocationPhase::Signs {
                kernel
            } else {
                0
            })
        );
        assert_eq!(
            memory.retained_bytes(),
            0,
            "every partial allocation retires on refusal"
        );
    }
}
