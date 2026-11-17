//! Exact adverse-runtime observation at the original whole-workset admission.
//! Signed binding units, Host capacities, IMAGE/grants and policy stay unchanged.
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    execution::Refusal,
    execution_workset::{PreparedLocalKernels, PreparedLocalWorkset},
};
use sha2::{Digest, Sha256};
use std::{io::Write, sync::atomic::Ordering::SeqCst};

struct Demand<'a> {
    pool: &'a str,
    class: &'a str,
    units: u32,
}

// Streaming semantic equality witness avoids cloning the original Host/catalog
// merely to prove an atomic refusal. The original empty admission vectors are
// independently asserted before/after; no trace/hash supplies authority.
fn ledger_digest(owner: &c::ResourceAdmissionOwner) -> [u8; 32] {
    struct Sink(Sha256);
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink(Sha256::new());
    serde_json::to_writer(&mut sink, owner).expect("fixed CPU ledger digest");
    sink.0.finalize().into()
}

fn no_started_effects() {
    assert!(!crate::effects::enabled(), "no Started evidence was opened");
    assert_eq!(crate::effects::CAPTURES.load(SeqCst), 0);
    assert_eq!(crate::effects::WAITS.load(SeqCst), 0);
    assert_eq!(crate::effects::PROGRAMS.load(SeqCst), 0);
    assert_eq!(crate::effects::frame_count(), 0);
}

/// Consume the ACTUAL complete factory result. This negative case never calls
/// bind/Start or edits the signed request, resource owner inventory or grant.
/// The sole changed input is explicitly simulated current availability.
pub fn refuse<T: PreparedLocalKernels>(
    prepared: PreparedLocalWorkset<'_, T, 3>,
    resources: &mut c::ResourceAdmissionOwner,
) {
    no_started_effects();
    assert!(resources.admissions().is_empty());
    assert!(resources.assignments().is_empty());
    let original = ledger_digest(resources);
    let (observations, selected, demand, available) = {
        let actual = resources.host();
        let mut demands = heapless::Vec::<Demand<'_>, 6>::new();
        for fragment in prepared.fragments() {
            let metadata = fragment.metadata();
            assert_eq!(metadata.host, actual.host_id.as_str());
            assert_eq!(metadata.boot, actual.boot_id.as_str());
            assert_eq!(metadata.offer_generation, actual.offer_generation.0);
            for binding in b::PlainFragmentResources::new(metadata.resource_bindings) {
                let binding = binding.expect("original checked plain resource binding");
                if let Some(prior) = demands.iter_mut().find(|d| d.pool == binding.pool) {
                    assert_eq!(prior.class, binding.class);
                    prior.units = prior
                        .units
                        .checked_add(binding.units)
                        .expect("checked aggregate demand");
                } else {
                    demands
                        .push(Demand {
                            pool: binding.pool,
                            class: binding.class,
                            units: binding.units,
                        })
                        .unwrap_or_else(|_| panic!("original finite pool inventory"));
                }
            }
        }
        let selected = demands
            .iter()
            .position(|d| d.units > 0)
            .expect("whole Source must request a positive resource");
        let demand = demands[selected].units;
        let available = demand - 1;
        let mut observations = heapless::Vec::<c::ResourceObservation, 6>::new();
        for (index, demand) in demands.iter().enumerate() {
            let mut pools = actual
                .resources
                .iter()
                .filter(|p| p.pool_id.as_str() == demand.pool);
            let pool = pools.next().expect("original actual pool");
            assert!(pools.next().is_none(), "unique actual pool identity");
            assert_eq!(pool.class_id.as_str(), demand.class);
            assert!(pool.compute.is_none() && pool.content.is_none());
            assert!(
                demand.units <= pool.capacity_units,
                "baseline authored demand must fit the unchanged Host pool"
            );
            let unreserved = if index == selected {
                available
            } else {
                pool.capacity_units
            };
            observations
                .push(c::ResourceObservation {
                    host_id: actual.host_id.clone(),
                    boot_id: actual.boot_id.clone(),
                    offer_generation: actual.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: c::ResourceHealth::Ready,
                    unreserved_units: unreserved,
                    utilized_units: pool.capacity_units - unreserved,
                    sign_id: format!(
                        "{}/cpu-current-shortage/{}",
                        actual.boot_id.as_str(),
                        demand.pool
                    )
                    .into(),
                })
                .unwrap_or_else(|_| panic!("original finite observation inventory"));
        }
        (observations, selected, demand, available)
    };
    // Same accounting as production observe_held: the fixed outer table is
    // inline, and each independently owned identity String contributes its
    // ACTUAL capacity. No invented bytes are added to manufacture shortage.
    let observation_backing: usize = observations
        .iter()
        .map(|o| {
            o.host_id.0.capacity()
                + o.boot_id.0.capacity()
                + o.pool_id.0.capacity()
                + o.class_id.0.capacity()
                + o.sign_id.0.capacity()
        })
        .sum();
    match prepared.admit::<()>(resources, &observations, observation_backing) {
        Err(Refusal::Resources(b::DelegatedResourceRefusal::Admission(
            c::ResourceAdmissionRefusal::Overcommitted,
        ))) => {}
        Err(other) => panic!("wrong shortage refusal: {other:?}"),
        Ok(_) => panic!("one-unit runtime shortage admitted the whole Source"),
    }
    assert_eq!(ledger_digest(resources), original);
    assert!(resources.admissions().is_empty());
    assert!(resources.assignments().is_empty());
    no_started_effects();
    println!("CPU_FULL_FACTORY_SHORTAGE pool={} authored_aggregate_demand={} observed_unreserved={} deficit=1 unchanged_Host_and_signed_bindings=true ledger_digest_unchanged=true admissions=0 assignments=0 Start=false provider_effects=0 observation_heap_backing={}",observations[selected].pool_id.as_str(),demand,available,observation_backing);
}
