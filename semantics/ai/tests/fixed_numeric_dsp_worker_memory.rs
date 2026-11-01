#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_dsp_catalog::*, operation_owners::fixed_numeric::FixedNumericOperationFactory,
};
use conduit_composite::KernelOperationFactory;
use conduit_kernel::HostedValueStore;
#[path = "common/fixed_dsp.rs"]
mod fixture;
// Keep the shared probe in this module so the measured retained-byte delta is
// directly available. This compares real allocations with the owner's receipt,
// independently of its codec-capacity arithmetic.
include!("../../../architecture/plot/tests/common/allocation_probe.rs");
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
#[test]
fn every_dsp_and_conversion_worker_reports_actual_retained_allocations() {
    let mut plans = Vec::new();
    for flow in [false, true] {
        for op in [
            FixedDspOperation::RealDft320,
            FixedDspOperation::MagnitudeSquared161,
            FixedDspOperation::Log10_18,
            FixedDspOperation::Log10_1,
            FixedDspOperation::Dot160,
            FixedDspOperation::Sqrt1,
        ] {
            plans.push(fixture::plan(op, flow));
        }
        for op in [
            IntegerConversion::U16,
            IntegerConversion::I16Vector80,
            IntegerConversion::I16Vector160,
            IntegerConversion::FiniteVector(1),
            IntegerConversion::FiniteVector(18),
            IntegerConversion::FiniteVector(20),
            IntegerConversion::FiniteVector(160),
            IntegerConversion::FiniteVector(256),
            IntegerConversion::FiniteVector(320),
        ] {
            plans.push(fixture::conversion_plan(op, flow));
        }
    }
    assert_eq!(plans.len(), 30);
    for plan in plans {
        let gear = fixture::gear(&plan);
        let owner =
            FixedNumericOperationFactory::for_plan(&plan, &std::collections::BTreeMap::new())
                .unwrap()
                .into_iter()
                .find(|f| f.implementation_id().as_str() == DSP_IMPLEMENTATION)
                .unwrap();
        let mut values = HostedValueStore::new(1, 16384, 16384).unwrap();
        let (prepared, observation) = observe(|| owner.prepare_worker(gear, &mut values).unwrap());
        assert!(observation.live_bytes > 0);
        assert_eq!(
            prepared.retained_bytes(),
            observation.live_bytes,
            "{}",
            gear.kind_id.as_str()
        );
        let mut foreign = gear.clone();
        foreign.artifact_id = "foreign/dsp".into();
        assert!(owner.prepare_worker(&foreign, &mut values).is_err());
        drop(prepared);
    }
}
