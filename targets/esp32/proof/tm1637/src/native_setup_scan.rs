//! Transient selected scan custody in the same Root control/workspace lane.
//! Preparation never dispatches. Retirement preserves original evidence until
//! commit-last readback, actual AP return and exact Root scope release finish.
use super::{NativeRoot, WorkspacePermit};
use crate::native_network::scan::ScanProvider;
use crate::native_scan::{NativeScan, RetiredNativeScan};
use alloc::boxed::Box;
use conduit_network_join_call::scan::PreparedScanKernelStorage;

pub(crate) enum Job {
    Held(Box<NativeScan<ScanProvider>>),
    Running(Box<NativeScan<ScanProvider>>),
    Retired {
        record: Box<RetiredNativeScan<ScanProvider>>,
        root_finished: bool,
    },
}
impl NativeRoot {
    pub(super) fn prepare_recovery_scan(
        &mut self,
        window: Option<&WorkspacePermit<'_>>,
    ) -> &'static [u8] {
        // The parent already checked evidence capacity and the current Body.
        // A public control poll without independently admitted workspace cannot
        // obtain a scan owner, even when saved-profile policy is exhausted.
        let Some(window) = window.filter(|w| w.held()) else {
            return b"scan-awaiting-workspace";
        };
        if self.scan.is_some() || !self.network_workspace_scope_current() {
            return b"scan-owner-pending";
        }
        let now = embassy_time::Instant::now().as_millis();
        if !self.network.recovery.attempt_due(now)
            || self
                .network
                .recovery
                .check_scan_retirement(self.network.generation(), now)
                .is_err()
        {
            return b"scan-recovery-ineligible";
        }
        let Some(generation) = self.offer_generation.checked_add(1) else {
            return b"scan-offer-generation-exhausted";
        };
        let image = match crate::native_scan_source::load() {
            Ok(image) => image,
            Err(_) => return b"scan-source-refused",
        };
        let pins = match crate::native_scan_source::identity_ref() {
            Ok(pins) => pins,
            Err(_) => return b"scan-source-pins-refused",
        };
        // All fallible container/execution backing precedes actual AP transfer.
        // Allocation remains above the parent's physically retained release floor.
        let backing = match crate::native_box_backing::reserve_box::<NativeScan<ScanProvider>>() {
            Ok(backing) => backing,
            Err(()) => return b"scan-owner-capacity",
        };
        let storage = match PreparedScanKernelStorage::try_new() {
            Ok(storage) => storage,
            Err(_) => return b"scan-execution-capacity",
        };
        let ready = match self.network.prepare_scan(&self.host, &self.boot, now) {
            Ok(ready) => ready,
            Err(reason) => return reason.as_bytes(),
        };
        let mut scan = Box::write(backing, NativeScan::new(*ready));
        let installed = scan.install_held_with_storage(
            &image,
            pins,
            conduit_core::OfferGeneration(generation),
            &mut self.issuer,
            storage,
            || window.held().then_some(()).ok_or(()),
        );
        self.offer_generation = generation;
        if installed.is_err() {
            if let Ok(ready) = scan.take_unselected_ready() {
                match self.network.return_unselected_scan(ready) {
                    Ok(()) => return b"scan-preparation-refused",
                    Err(ready) => *scan = NativeScan::new(ready),
                }
            }
        }
        // An irreversible refusal retains all original issued custody; it is
        // never converted to a new attempt or reported as Playing.
        self.scan = Some(Job::Held(scan));
        if installed.is_ok() {
            b"scan-prepared"
        } else {
            b"scan-custody-retained"
        }
    }
    pub(super) fn release_recovery_scan(&mut self) -> Result<(), ()> {
        let Some(job) = self.scan.take() else {
            return Err(());
        };
        match job {
            Job::Held(mut scan) => {
                let result = scan.release();
                self.scan = Some(if result.is_ok() {
                    Job::Running(scan)
                } else {
                    Job::Held(scan)
                });
                result
            }
            other => {
                self.scan = Some(other);
                Err(())
            }
        }
    }
    pub(super) async fn advance_recovery_scan(&mut self) {
        if let Some(Job::Running(scan)) = self.scan.as_mut() {
            let _ = scan.drive().await;
            if scan.restore_for_retirement().await.is_err() {
                return;
            }
            // Reserve the destination before relinquishing the live executor.
            let Ok(backing) = crate::native_box_backing::reserve_box() else {
                return;
            };
            let Ok(record) = scan.take_retired() else {
                return;
            };
            self.scan = Some(Job::Retired {
                record: Box::write(backing, record),
                root_finished: false,
            });
        }
        let _ = self.finish_recovery_scan();
    }
    fn finish_recovery_scan(&mut self) -> Result<(), &'static str> {
        #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
        {
            let Some(Job::Retired {
                record,
                root_finished,
            }) = self.scan.as_mut()
            else {
                return Err("scan-not-retired");
            };
            let generation = record
                .evidence()
                .inspection()
                .scope
                .base_provider_generation;
            let now = embassy_time::Instant::now().as_millis();
            self.network
                .recovery
                .check_scan_retirement(generation, now)
                .map_err(|_| "scan-retirement-policy")?;
            if record.provider.is_some() && !self.network.can_reclaim_restored() {
                return Err("scan-ap-return-pending");
            }
            record.commit_evidence()?;
            // Original evidence stays in this job even if physical return or
            // Root release refuses. Each completed handoff is represented once.
            if let Some(provider) = record.provider.take() {
                if let Err(provider) = self.network.reclaim_restored_scan(provider) {
                    record.provider = Some(provider);
                    return Err("scan-ap-return-refused");
                }
            }
            if !*root_finished {
                self.issuer
                    .finish_scan_play(&record.active, record.committed().expect("committed"))
                    .map_err(|_| "scan-root-retirement-refused")?;
                *root_finished = true;
            }
            let renewed = crate::native_credentials::renew_after_scan(
                &mut self.credentials,
                record.committed().expect("committed"),
                now,
            )
            .is_ok();
            self.network
                .recovery
                .scan_retired(generation, now, renewed)
                .map_err(|_| "scan-retirement-transition")?;
            self.scan = None;
            Ok(())
        }
        #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
        {
            Err("scan-durable-custody-required")
        }
    }
}
