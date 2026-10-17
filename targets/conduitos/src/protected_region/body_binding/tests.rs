use super::*;
use crate::native_workset::{self, NativePlot};

fn fixture() -> (
    BodyPlan,
    BodyPlayIdentity,
    HostId,
    BootId,
    ResidentPlot,
    ExecutionRegionId,
) {
    let (ids, offer) = native_workset::tests::fixture();
    let wake = native_workset::tests::wake(&native_workset::inventory());
    let prepared = native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
    let plot = native_workset::resident(NativePlot::KeyboardCanvas).unwrap();
    let partition = prepared
        .plan()
        .plots
        .iter()
        .find(|p| p.plot == plot)
        .unwrap();
    let fragment = &partition.plan.fragments[0];
    assert_eq!(fragment.execution_regions.len(), 1);
    assert_eq!(fragment.execution_regions[0].admitted_placements.len(), 4);
    assert!(!fragment.execution_regions[0].isolation_required);
    let host = fragment.host_id.clone();
    let boot = fragment.boot_id.clone();
    let region = fragment.execution_regions[0].region_id.clone();
    let plan = prepared.plan().clone();
    let play = BodyPlayIdentity::bind(&plan, 7);
    (plan, play, host, boot, plot, region)
}

#[test]
fn real_native_partition_binds_the_actual_body_play() {
    let (plan, play, host, boot, plot, region) = fixture();
    let bound = BodyRegionBinding::admit(
        &plan,
        &play,
        &plot,
        &host,
        &boot,
        &region,
        ProtectionDomainId(1),
    )
    .unwrap();
    assert_eq!(bound.active, play);
    assert_eq!(bound.active.plan_id, plan.plan_id);
    assert_ne!(bound.active.plan_id, bound.partition_plan);
    assert_eq!(bound.plot, plot);
}

#[test]
fn forged_play_and_wrong_host_boot_partition_region_or_domain_are_refused() {
    let (plan, play, host, boot, plot, region) = fixture();
    let admit =
        |p: &BodyPlan,
         a: &BodyPlayIdentity,
         h: &HostId,
         b: &BootId,
         plot: &ResidentPlot,
         r: &ExecutionRegionId,
         d| { BodyRegionBinding::admit(p, a, plot, h, b, r, ProtectionDomainId(d)) };
    let mut forged = play.clone();
    forged.play_sequence += 1;
    assert!(admit(&plan, &forged, &host, &boot, &plot, &region, 1).is_err());
    let mut changed = plan.clone();
    changed.workload_revision += 1;
    assert!(admit(&changed, &play, &host, &boot, &plot, &region, 1).is_err());
    for (h, b, p, r, d) in [
        (
            "host/other".into(),
            boot.clone(),
            plot.clone(),
            region.clone(),
            1,
        ),
        (
            host.clone(),
            "boot/other".into(),
            plot.clone(),
            region.clone(),
            1,
        ),
        (
            host.clone(),
            boot.clone(),
            native_workset::resident(NativePlot::Patchbay).unwrap(),
            region.clone(),
            1,
        ),
        (
            host.clone(),
            boot.clone(),
            plot.clone(),
            "region/absent".into(),
            1,
        ),
        (host.clone(), boot.clone(), plot.clone(), region.clone(), 0),
    ] {
        assert!(admit(&plan, &play, &h, &b, &p, &r, d).is_err());
    }
}
