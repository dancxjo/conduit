use super::*;
use conduit_core::{
    ArchitectureBaseId, ArchitectureBaseKind, ComputeReservation, ComputeServiceGuarantee,
    ComputeTopologyGroup,
};

fn pool(name: &str) -> PoolUsage {
    PoolUsage::new(&ResourceOffer {
        pool_id: name.into(),
        class_id: "conduit.resource/execution-lane@1".into(),
        capacity_units: 4,
        content: None,
        compute: Some(ComputePoolContract {
            service_guarantee: ComputeServiceGuarantee::Shared,
            architecture_base_id: ArchitectureBaseId::from("base"),
            architecture_base_kind: ArchitectureBaseKind::HostedOs,
            topology_groups: vec![ComputeTopologyGroup {
                group_id: "group".into(),
                lane_capacity: 2,
                numa_domain: None,
                cache_domain: None,
                performance_class: None,
                nominal_clock_hz: None,
            }],
        }),
    })
    .unwrap()
}
fn binding(name: &str, units: u32) -> ResourceBinding {
    ResourceBinding {
        pool_id: name.into(),
        class_id: "conduit.resource/execution-lane@1".into(),
        units,
        protected: None,
        content: None,
        compute: Some(ComputeReservation {
            selected_lanes: units,
            service_guarantee: ComputeServiceGuarantee::Shared,
            architecture_base_id: "base".into(),
            architecture_base_kind: ArchitectureBaseKind::HostedOs,
            topology_group_id: Some("group".into()),
            performance_class: None,
            nominal_clock_hz: None,
        }),
    }
}
#[test]
fn cumulative_group_shortage_is_atomic_and_release_reuses_capacity() {
    let mut pools = vec![pool("lanes")];
    let initial = pools.clone();
    let capacity = pools[0].groups.capacity();
    PoolUsage::reserve_all(&mut pools, &[binding("lanes", 1)]).unwrap();
    let occupied = pools.clone();
    assert!(PoolUsage::reserve_all(&mut pools, &[binding("lanes", 2)]).is_err());
    assert_eq!(pools, occupied);
    PoolUsage::reserve_all(&mut pools, &[binding("lanes", 1)]).unwrap();
    assert_eq!(pools[0].used_units, 2);
    assert_eq!(pools[0].groups[0].used_units, 2);
    assert!(PoolUsage::reserve_all(&mut pools, &[binding("lanes", 1)]).is_err());
    PoolUsage::release_all(&mut pools, &[binding("lanes", 2)]).unwrap();
    assert_eq!(pools, initial);
    assert_eq!(pools[0].groups.capacity(), capacity);
    assert!(PoolUsage::release_all(&mut pools, &[binding("lanes", 1)]).is_err());
    assert_eq!(pools, initial);
}
#[test]
fn late_pool_failure_and_unknown_group_change_no_counters() {
    let mut pools = vec![pool("a"), pool("b")];
    PoolUsage::reserve_all(&mut pools, &[binding("b", 2)]).unwrap();
    let occupied = pools.clone();
    assert!(PoolUsage::reserve_all(&mut pools, &[binding("a", 1), binding("b", 1)]).is_err());
    assert_eq!(pools, occupied);
    let mut unknown = binding("a", 1);
    unknown.compute.as_mut().unwrap().topology_group_id = Some("unknown".into());
    assert!(PoolUsage::reserve_all(&mut pools, &[unknown]).is_err());
    assert_eq!(pools, occupied);
    assert!(PoolUsage::reserve_all(&mut pools, &[binding("absent", 1)]).is_err());
    assert_eq!(pools, occupied);
}
