//! Runs actual Root scan selection/issuance/dispatch with an inert counted
//! provider. Linked fixture material is prepared before allocation observations.
use crate::{allocation_probe, native_authority::RootIssuer, native_scan::NativeScan};
use conduit_core as c;
use conduit_net::*;
use conduit_network_join_call::{installation::NetworkNativeIdentity, scan::*};
use conduit_plot::{
    CheckedConnectionRef, CheckedGearRef, LinkedPlanningFactsRef, PlanningOriginRef,
};
use std::{cell::Cell, rc::Rc, sync::OnceLock};

static MATERIAL: OnceLock<(Vec<u8>, LinkedPlanningFactsRef<'static>)> = OnceLock::new();
fn facts() -> Result<LinkedPlanningFactsRef<'static>, conduit_plot::flat_image::ImageRefusal> {
    Ok(MATERIAL.get().unwrap().1)
}
fn image() -> conduit_plot::flat_image::indexed::IndexedSourceImage<'static> {
    let (bytes, _) = MATERIAL.get_or_init(|| {
        let source = Box::leak(Box::new(
            conduit_environment_make::compile_network_scan().unwrap(),
        ));
        let bytes = conduit_plot::flat_image::indexed::encode(source).unwrap();
        let gears = Box::leak(
            source
                .gears
                .iter()
                .map(CheckedGearRef::from)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let connections = Box::leak(
            source
                .connections
                .iter()
                .map(CheckedConnectionRef::from)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let provenance = Box::leak(
            source
                .provenance
                .iter()
                .map(|p| PlanningOriginRef {
                    gear_id: p.gear_id.as_str(),
                    source_span: p.source_span,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        (
            bytes,
            LinkedPlanningFactsRef {
                source_document_id: source.source_document_id.as_str(),
                checked_plot_id: source.checked_plot_id.as_str(),
                expanded_plot_id: source.expanded_plot_id.as_str(),
                name: &source.name,
                completion: source.completion,
                gears,
                connections,
                provenance,
            },
        )
    });
    // SAFETY: exact Make-checked Source and immutable matching test-only facts.
    unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(bytes)
            .unwrap()
            .with_make_planning_facts(facts)
    }
}
#[derive(Default)]
struct State {
    now: Cell<u64>,
    scans: Cell<usize>,
    cleanups: Cell<usize>,
    cleanup_fail: Cell<bool>,
    scan_fail: Cell<bool>,
    pending: Cell<bool>,
}
struct CpuScan(Rc<State>);
impl NetworkScanProvider for CpuScan {
    fn now_ms(&self) -> u64 {
        self.0.now.get()
    }
    async fn scan(
        &mut self,
        request: NetworkScanRequest,
        output: &mut [ScanObservation; 16],
    ) -> Result<ScanCompletion, ScanFailure> {
        self.0.scans.set(self.0.scans.get() + 1);
        if self.0.pending.get() {
            core::future::pending::<()>().await;
        }
        if self.0.scan_fail.get() {
            return Err(ScanFailure::Rejected);
        }
        self.0.now.set(2);
        for r in output {
            *r = ScanObservation::new(b"cpu-observed", ObservedNetworkSecurity::Unknown, -51, 2)?;
        }
        Ok(ScanCompletion {
            generation: request.generation,
            completed_ms: 2,
            count: 16,
            total: 17,
        })
    }
    async fn cleanup(&mut self, _: u64) -> Result<(), ScanFailure> {
        self.0.cleanups.set(self.0.cleanups.get() + 1);
        if self.0.cleanup_fail.get() {
            Err(ScanFailure::Rejected)
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_root_scan_failure_and_cancel_keep_cleanup_and_never_fabricate_observations() {
    use core::{
        future::Future,
        task::{Context, Waker},
    };
    let image = image();
    let f = facts().unwrap();
    let pins = (f.source_document_id, f.checked_plot_id, f.expanded_plot_id);
    for pending in [false, true] {
        let state = Rc::new(State::default());
        state.now.set(1);
        state.pending.set(pending);
        state.scan_fail.set(!pending);
        let mut root = RootIssuer::fixture("cpu/scan-host".into(), "cpu/scan-boot".into());
        let mut scan = NativeScan::new(ready(state.clone()));
        scan.install_held(&image, pins, c::OfferGeneration(1), &mut root, || Ok(()))
            .unwrap();
        scan.release().unwrap();
        if pending {
            let mut future = core::pin::pin!(scan.drive());
            assert!(future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
        } else {
            assert!(embassy_futures::block_on(scan.drive()).is_err());
        }
        assert_eq!(state.scans.get(), 1);
        assert!(scan.take_retired().is_err());
        embassy_futures::block_on(scan.restore_for_retirement()).unwrap();
        let retired = scan.take_retired().unwrap();
        assert!(Rc::ptr_eq(&retired.provider.0, &state));
        assert!(!retired.evidence_seen);
        assert!(retired.evidence.completed_output().is_none());
        assert!(retired.evidence.completed_observations().is_none());
        assert_eq!(
            retired.evidence.inspection().lifecycle,
            c::CapabilityLifecycle::Revoked
        );
        assert_eq!(retired.evidence.inspection().in_flight_operations, 0);
        assert_eq!(state.cleanups.get(), 1);
        assert!(retired.signs.events().count() > 0);
        assert_eq!(
            retired.evidence.inspection().scope.active_play_id,
            retired.active.active_play_id
        );
    }
}
fn ready(state: Rc<State>) -> ReadyNetworkScanBase<CpuScan> {
    let native = NetworkNativeIdentity {
        host: "cpu/scan-host".into(),
        boot: "cpu/scan-boot".into(),
        base: "cpu/base".into(),
        instance: "cpu/instance".into(),
        provider_generation: 1,
        resource: "cpu/radio".into(),
        resource_generation: c::ResourceGenerationId("cpu/radio/1".into()),
        envelope: "cpu/envelope".into(),
        artifact: "cpu/scan-artifact".into(),
        implementation: "cpu/scan@1".into(),
        base_implementation: "cpu/scan-base@1".into(),
    };
    // SAFETY: unique inert CPU provider; no physical radio/SDK is claimed.
    unsafe {
        ReadyNetworkScanBase::new(
            native,
            NetworkScanRequest {
                generation: 1,
                scan_deadline_ms: 15_000,
                cleanup_deadline_ms: 20_000,
            },
            CpuScan(state),
        )
    }
    .unwrap_or_else(|_| panic!("valid CPU scan owner"))
}
#[test]
fn actual_root_scan_admission_dispatch_and_retirement_preserve_original_custody() {
    let image = image();
    let f = facts().unwrap();
    let pins = (f.source_document_id, f.checked_plot_id, f.expanded_plot_id);
    let state = Rc::new(State::default());
    state.now.set(1);
    let mut root = RootIssuer::fixture("cpu/scan-host".into(), "cpu/scan-boot".into());
    let mut scan = NativeScan::new(ready(state.clone()));
    assert!(scan
        .install_held(
            &image,
            (pins.0, "foreign/checked", pins.2),
            c::OfferGeneration(1),
            &mut root,
            || panic!("wrong Source must refuse first")
        )
        .is_err());
    assert_eq!(state.scans.get(), 0);
    assert!(scan
        .install_held(&image, pins, c::OfferGeneration(1), &mut root, || Err(()))
        .is_err());
    assert_eq!(state.scans.get(), 0);
    // Same original owner and Root remain available after failed admission.
    let returned_ready = scan
        .take_unselected_ready()
        .unwrap_or_else(|_| panic!("unissued actual owner"));
    assert!(scan.take_unselected_ready().is_err());
    let mut scan = NativeScan::new(returned_ready);
    assert!(scan
        .install_held(&image, pins, c::OfferGeneration(1), &mut root, || Ok(()))
        .is_ok());
    assert!(embassy_futures::block_on(scan.drive()).is_err());
    assert_eq!(state.scans.get(), 0);
    assert!(scan.take_retired().is_err());
    assert!(scan.take_unselected_ready().is_err());
    scan.release().unwrap();
    let (result, memory) = allocation_probe::observe(|| embassy_futures::block_on(scan.drive()));
    result.unwrap();
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(state.scans.get(), 1);
    assert!(scan.take_retired().is_err());
    state.cleanup_fail.set(true);
    assert!(embassy_futures::block_on(scan.restore_for_retirement()).is_err());
    assert!(scan.take_retired().is_err());
    state.cleanup_fail.set(false);
    embassy_futures::block_on(scan.restore_for_retirement()).unwrap();
    let (retired, memory) = allocation_probe::observe(|| scan.take_retired().unwrap());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(retired.evidence_seen);
    assert!(Rc::ptr_eq(&retired.provider.0, &state));
    assert_eq!(retired.evidence.completed_output().unwrap().len(), 728);
    assert_eq!(retired.evidence.completed_observations().unwrap().len(), 16);
    assert_eq!(
        retired.evidence.completed_observations().unwrap()[0].security,
        ObservedNetworkSecurity::Unknown
    );
    assert_eq!(
        retired.evidence.inspection().scope.active_play_id,
        retired.active.active_play_id
    );
    assert_eq!(
        retired.evidence.inspection().lifecycle,
        c::CapabilityLifecycle::Revoked
    );
    assert!(retired.signs.events().count() > 0);
    assert_eq!(state.cleanups.get(), 2);
    assert!(scan.take_retired().is_err());
    // Returning radio ownership/evidence does not commit it or renew Root.
    let mut next = NativeScan::new(ready(Rc::new(State::default())));
    assert!(next
        .install_held(&image, pins, c::OfferGeneration(2), &mut root, || panic!(
            "spent Root slot must refuse before admission"
        ))
        .is_err());
}
