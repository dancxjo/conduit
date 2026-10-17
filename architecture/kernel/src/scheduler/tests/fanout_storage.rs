//! Refused reference preparation must not leak an unpublished fan-out value.
use super::*;
use crate::StorageError;

struct LimitedReferences {
    inner: FixedValueStore<2, 8>,
    limit: u16,
}
impl ValueStorage for LimitedReferences {
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.inner.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        self.inner.store(bytes)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(value)
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(value)
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        if self.inner.reference_count(value)? >= self.limit {
            return Err(StorageError::ReferenceOverflow);
        }
        self.inner.retain(value)
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        self.inner.release(value)
    }
    fn clear(&mut self) {
        self.inner.clear();
    }
}

type Fanout = FixedScheduler<Driver, LimitedReferences, FixedSignLog<32>, 4, 3, PORTS, 3, 8, 3>;
fn scheduler(track: AssignedConnectionTrack) -> Fanout {
    let mut routes = FixedRoutes::new(PORTS as u16);
    let targets = core::array::from_fn::<_, 3, _>(|i| RouteTarget {
        cord: CordId(i as u16),
        sink: crate::CordEndpoint::local(NodeId((i + 1) as u16), PortId(0)),
    });
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 3 },
            &targets,
        )
        .unwrap();
    routes.seal().unwrap();
    Fanout::new(
        core::array::from_fn(|i| {
            node([
                if i == 0 {
                    None
                } else {
                    Some(CordId((i - 1) as u16))
                },
                None,
            ])
        }),
        core::array::from_fn(|i| {
            CordSpec::local(
                CordId(i as u16),
                (NodeId(0), PortId(0)),
                (NodeId((i + 1) as u16), PortId(0)),
                CordCapacity {
                    slot_start: i as u16,
                    item_capacity: 1,
                    byte_capacity: 8,
                    pressure_policy: Default::default(),
                },
            )
            .with_track(track)
        }),
        routes,
        core::array::from_fn(|_| Driver::BlockedSink { cancelled: false }),
        LimitedReferences {
            inner: FixedValueStore::new(16).unwrap(),
            limit: 2,
        },
        FixedSignLog::new(32 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}
fn publish(scheduler: &mut Fanout, track: AssignedConnectionTrack) -> Result<(), SchedulerError> {
    match track {
        AssignedConnectionTrack::Quiescence => scheduler.emit_quiescence_transitions().map(|_| ()),
        AssignedConnectionTrack::NormalClose => scheduler.close_outputs(0),
        AssignedConnectionTrack::AbnormalTerminal => scheduler
            .abnormally_terminate_outputs(0, PortId(0), CanonicalValue::new(b"lost").unwrap())
            .map(|_| ()),
        _ => unreachable!(),
    }
}
fn refusal_and_retry(track: AssignedConnectionTrack) {
    let mut scheduler = scheduler(track);
    scheduler.arm_quiescence_outputs(0).unwrap();
    let armed = scheduler.cords.map(|cord| cord.quiescence_armed);
    let ready = core::array::from_fn::<_, 4, _>(|node| scheduler.ready.contains(node));
    let signs = scheduler.signs.events().count();
    assert_eq!(
        publish(&mut scheduler, track),
        Err(SchedulerError::Storage(StorageError::ReferenceOverflow))
    );
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    assert_eq!(scheduler.signs.events().count(), signs);
    assert_eq!(scheduler.cords.map(|cord| cord.quiescence_armed), armed);
    assert_eq!(
        core::array::from_fn::<_, 4, _>(|node| scheduler.ready.contains(node)),
        ready
    );
    for cord in &scheduler.cords {
        assert_eq!(cord.len, 0);
        assert!(!cord.producer_closed);
    }
    scheduler.values.limit = u16::MAX;
    publish(&mut scheduler, track).unwrap();
    let value = scheduler.peek(0).unwrap().unwrap();
    assert_eq!(scheduler.values.reference_count(value), Ok(3));
    for cord in 0..3 {
        assert_eq!(scheduler.peek(cord).unwrap(), Some(value));
        let value = scheduler.pop(cord).unwrap();
        scheduler.values.release(value).unwrap();
    }
    assert_eq!(scheduler.values.used_items(), 0);
}
#[test]
fn refused_quiescence_fanout_reclaims_unpublished_references() {
    refusal_and_retry(AssignedConnectionTrack::Quiescence);
}
#[test]
fn refused_normal_close_fanout_reclaims_unpublished_references() {
    refusal_and_retry(AssignedConnectionTrack::NormalClose);
}
#[test]
fn refused_abnormal_fanout_reclaims_unpublished_references() {
    refusal_and_retry(AssignedConnectionTrack::AbnormalTerminal);
}
