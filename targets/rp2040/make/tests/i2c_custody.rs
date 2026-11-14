//! Actual firmware controller lease, using a fake configured physical leaf.
extern crate alloc;
extern crate self as conduit_rp2040_i2c;
use conduit_i2c_transaction::{I2cDisposition, I2cProvider, I2cTransaction};
use std::{
    future::Future,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;
mod config {
    pub type Bus = ();
    pub const I2C_CONTROLLER: u8 = 1;
    pub const I2C_BASE_IMPLEMENTATION: Option<&str> = Some("rp2040/i2c@1");
    pub const I2C_CALL_MAXIMUM_OPERATIONS: u32 = 1024;
    pub const I2C_CALL_ADDRESS_RANGE: Option<(u8, u8)> = Some((112, 118));
    pub const CONFIGURATION_ID: &str = "fixture/config";
    pub const IMAGE_ID: &str = "fixture/image";
    pub const RESIDENT_WORK_POOL: &str = "fixture/work";
    pub const RESIDENT_WORK_UNITS: u32 = 5000;
    pub static I2C_OFFER_IMAGE: Option<&[u8]> = Some(&[1]); // leaf, not a wire image proof
}
pub struct Rp2040I2c<'a, T> {
    calls: Arc<AtomicUsize>,
    revoked: bool,
    marker: PhantomData<&'a T>,
}
impl<T> Rp2040I2c<'_, T> {
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }
}
impl<T> I2cProvider for Rp2040I2c<'_, T> {
    fn transact(
        &mut self,
        _: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert!(owner::protocol_revision() > 0);
        if self.revoked {
            return Err(I2cDisposition::ProviderLost);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        input.fill(7);
        Ok(input.len())
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}
#[path = "../../firmware/host/src/i2c_owner.rs"]
mod owner;
use owner as i2c_owner;
#[path = "../../firmware/host/src/i2c_installation.rs"]
mod installation;
use installation as i2c_installation;
#[path = "../../firmware/host/src/native_publication.rs"]
mod native_publication;

#[test]
fn one_controller_lease_survives_cancellation_and_invalidates_cache_only_on_native_use() {
    critical_section::with(|_| {});
    assert!(owner::try_lease().is_none());
    assert_eq!(owner::native_available(), None);
    let calls = Arc::new(AtomicUsize::new(0));
    let bus = Rp2040I2c {
        calls: calls.clone(),
        revoked: false,
        marker: PhantomData,
    };
    let ((), cost) = probe::observe(|| owner::initialize(bus));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let lease = owner::try_lease().unwrap();
    let address = (&*lease as *const Rp2040I2c<'_, ()>) as usize;
    assert_eq!(std::mem::size_of_val(&lease), std::mem::size_of::<usize>());
    assert!(owner::try_lease().is_none());
    assert!(owner::try_native_lease().is_none());
    assert_eq!(owner::native_available(), None);
    drop(lease);
    let ((), cost) = probe::observe(|| drop(owner::try_native_lease().unwrap()));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(owner::protocol_revision(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut actual = conduit_core::HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: config::CONFIGURATION_ID.into(),
        bases: vec![],
        resources: vec![conduit_core::resource_offer(
            "fixture/work",
            conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
            5000,
        )],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    assert_eq!(owner::native_available(), Some(true));
    let (publication, publication_cost) =
        probe::observe(|| native_publication::prepare(&actual, true, 4096));
    let publication = publication.unwrap();
    assert_eq!(
        publication.offer_generation.0,
        actual.offer_generation.0 + 1
    );
    assert!(publication.capabilities.is_empty());
    assert_eq!(publication.resources.len(), 2);
    assert_eq!(publication.bases.len(), 1);
    assert_eq!(owner::native_available(), Some(true));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let tight = native_publication::prepare(&actual, true, publication_cost.peak_bytes).unwrap();
    assert_eq!(tight, publication);
    let (short, shortage) = probe::observe(|| {
        native_publication::prepare(&actual, true, publication_cost.peak_bytes - 1)
    });
    assert!(short.is_err());
    assert_eq!(shortage.allocations, shortage.deallocations);
    for nth in 1..=publication_cost.allocations {
        let (result, failed) =
            probe::observe_refusing(nth, || native_publication::prepare(&actual, true, 4096));
        assert!(result.is_err(), "publication allocation {nth}");
        assert_eq!(failed.refused_allocations, 1);
        assert_eq!(failed.allocations, failed.deallocations);
        assert_eq!(owner::native_available(), Some(true));
    }
    let withdrawn = native_publication::prepare(&publication, false, 4096).unwrap();
    assert!(withdrawn.bases.is_empty());
    assert_eq!(withdrawn.resources, actual.resources);
    assert_eq!(
        withdrawn.offer_generation.0,
        publication.offer_generation.0 + 1
    );
    println!("actual Root native publication preparation: {} requested B, {} B CPU component peak, {} fallible allocations; exact-size budget succeeds, one-byte shortage and all allocation faults preserve current owner and diagnostics",publication_cost.requested_bytes,publication_cost.peak_bytes,publication_cost.allocations);
    let mut unexpected = actual.clone();
    unexpected.resources.push(conduit_core::resource_offer(
        "unexpected/pool",
        "unexpected/class",
        1,
    ));
    let (result, rejected) =
        probe::observe(|| native_publication::prepare(&unexpected, true, 4096));
    assert!(matches!(result, Err(native_publication::Refusal::Context)));
    assert_eq!((rejected.allocations, rejected.reallocations), (0, 0));
    let mut wrong = actual.clone();
    wrong.profile = "foreign/config".into();
    assert!(matches!(
        installation::prepare(&wrong, 1024),
        Err(installation::Refusal::Context)
    ));
    assert!(matches!(
        installation::prepare(&actual, 0),
        Err(installation::Refusal::Capacity)
    ));
    let (prepared, allocation) = probe::observe(|| installation::prepare(&actual, 1024));
    let (ready, retained_bytes) = prepared.unwrap();
    assert_eq!(allocation.requested_bytes, retained_bytes);
    assert_eq!(allocation.reallocations, 0);
    let allocation_count = allocation.allocations;
    println!("CPU native I2C identity: {retained_bytes} retained heap bytes in {allocation_count} fallible allocations; no bus effect or authority grant");

    assert!(owner::try_lease().is_none());
    let (hint, cost) = probe::observe(|| ready.assigned_node_identity(1));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(hint.node, 1);
    assert_eq!(
        hint.kind,
        conduit_core::AssignedIdentity::from_text("machine/i2c/transact")
    );
    assert_eq!(
        hint.implementation,
        conduit_core::AssignedIdentity::from_text("conduitos/i2c-transaction@1")
    );
    assert_eq!(
        hint.artifact,
        conduit_core::AssignedIdentity::from_text(config::IMAGE_ID)
    );
    let (compact, projected) = probe::observe(|| ready.project_for_images(&actual, 4096));
    let compact = compact.unwrap();
    assert_eq!(projected.requested_bytes, compact.owned_bytes);
    assert_eq!(projected.reallocations, 0);
    let (short, short_cost) =
        probe::observe(|| ready.project_for_images(&actual, compact.owned_bytes - 1));
    assert!(matches!(
        short,
        Err(conduit_i2c_transaction::installation::I2cImageProjectionRefusal::Capacity)
    ));
    assert_eq!((short_cost.allocations, short_cost.reallocations), (0, 0));
    for nth in 1..=projected.allocations {
        let (result, failed) = probe::observe_refusing(nth, || {
            ready.project_for_images(&actual, compact.owned_bytes)
        });
        assert!(matches!(
            result,
            Err(conduit_i2c_transaction::installation::I2cImageProjectionRefusal::Allocation)
        ));
        assert_eq!(failed.allocations, failed.deallocations);
        assert_eq!(failed.refused_allocations, 1);
        assert!(owner::try_lease().is_none());
    }
    let mut foreign = actual.clone();
    foreign.boot_id = "wrong/boot".into();
    let (result, rejected) = probe::observe(|| ready.project_for_images(&foreign, 4096));
    assert!(matches!(
        result,
        Err(conduit_i2c_transaction::installation::I2cImageProjectionRefusal::Context)
    ));
    assert_eq!((rejected.allocations, rejected.reallocations), (0, 0));
    let contract = conduit_i2c_transaction::contract::I2cContract::prepare().unwrap();
    ready
        .append_to_advertisement(&mut actual, &contract)
        .unwrap();
    assert_eq!(actual.capabilities.len(), 1);
    assert_eq!(
        actual.capabilities[0].kind_id.as_str(),
        "machine/i2c/transact"
    );
    assert_eq!(actual.resources.len(), 2);
    assert_eq!(actual.bases.len(), 1);
    assert_eq!(compact.base, actual.bases[0]);
    assert_eq!(compact.resource, actual.resources[1]);
    println!("compact actual native publication: {} owned bytes, {} fallible allocations, all faults reclaimed; no catalog decoder or bus effect",compact.owned_bytes,projected.allocations);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(ready);
    assert_eq!(owner::protocol_revision(), 0);
    assert!(matches!(
        installation::prepare(&actual, retained_bytes - 1),
        Err(installation::Refusal::Capacity)
    ));
    for nth in 1..=allocation_count {
        let (result, cost) = probe::observe_refusing(nth, || installation::prepare(&actual, 1024));
        assert!(matches!(result, Err(installation::Refusal::Allocation)));
        assert_eq!(cost.refused_allocations, 1);
        assert_eq!(cost.allocations, cost.deallocations);
        assert_eq!(owner::protocol_revision(), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(owner::try_native_lease().is_some());
    }
    let request = I2cTransaction::new(112, &[0], 1).unwrap();
    let ((), cost) = probe::observe(|| {
        let mut native = owner::try_native_lease().unwrap();
        let mut input = [0];
        assert_eq!(native.transact(&request, &mut input), Ok(1));
        assert_eq!(input, [7]);
        assert_eq!(native.transact(&request, &mut input), Ok(1));
        assert_eq!(owner::protocol_revision(), 1);
        assert!(owner::try_lease().is_none());
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let pending = async {
        let _native = owner::try_native_lease().unwrap();
        core::future::pending::<()>().await;
    };
    assert!(std::mem::size_of_val(&pending) <= 3 * std::mem::size_of::<usize>());
    let mut pending = Box::pin(pending);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    let ((), cost) = probe::observe(|| drop(pending));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(owner::protocol_revision(), 1);
    let lease = owner::try_lease().unwrap();
    assert_eq!((&*lease as *const Rp2040I2c<'_, ()>) as usize, address);
    drop(lease);
    let ((), cost) = probe::observe(|| {
        let mut native = owner::try_native_lease().unwrap();
        native.revoke();
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(owner::protocol_revision(), 2);
    assert!(owner::try_native_lease().is_none());
    assert_eq!(owner::native_available(), Some(false));
    assert!(owner::try_lease().unwrap().is_revoked());
}
