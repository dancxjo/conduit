//! Preparation accounting for the application adapter, separate from kernel admission.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::publication::{
    PublicationBack, PublicationChannel, PublicationContract, PublicationSequence,
};
#[test]
fn channel_preparation_backing_is_measured_before_play() {
    for channel in PublicationChannel::ALL {
        let contract = PublicationContract::prepare(channel).unwrap();
        let order = PublicationSequence::prepare(2);
        let (back, allocation) =
            allocation_probe::observe(|| PublicationBack::prepare(&contract, 2, order).unwrap());
        assert_eq!(allocation.reallocations, 0);
        assert!(back.snapshot().is_none());
        let bound = conduit_plot::maximum_prepared_canonical_value_bytes(contract.value_type())
            .unwrap()
            .min(channel.maximum_bytes() as u32) as usize;
        assert_eq!(back.value_storage_bytes(), Some(2 * bound));
        eprintln!(
            "channel={channel:?} inline_bytes={} preparation={allocation:?}",
            core::mem::size_of::<PublicationBack>()
        );
    }
}
