//! Two-frame physical checkpoint over the actual sampler Face. This local
//! producer is not advertised as an installed ordinary Mask Plot/route.
use super::{execution::fore::*, NativeDisplay};
use alloc::boxed::Box;
use conduit_environment_runtime::display_mask::PreparedDisplayMask;
use conduit_kernel as k;
use embassy_time::{Duration, Instant, Timer};

pub(super) struct FaceFore {
    reader: &'static crate::native_face::Reader,
    mask: Box<PreparedDisplayMask>,
    body: [u8; 32],
    wake: [u8; 32],
    accepted: u32,
    next_page: u32,
}
impl LocalFore for FaceFore {
    async fn next(&mut self) -> Result<Option<Frame>, k::Failure> {
        if self.next_page == super::CONSTRUCTION_FRAMES {
            return Ok(None);
        }
        if self.next_page != self.accepted {
            return Err(fail(30));
        }
        if self.next_page != 0 {
            Timer::after(Duration::from_secs(3)).await;
        }
        // A finite preparation/work allowance: no tight polling, detached Face
        // borrow, received producer ID, invented measurement or silent retry.
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let now = Instant::now().as_millis();
            let output = self
                .reader
                .with_current(now, |face, status| {
                    let digest = conduit_core::assigned_plan_payload_digest;
                    if face
                        .basis
                        .body_id
                        .as_ref()
                        .map(|id| digest(id.as_str().as_bytes()))
                        != Some(self.body)
                        || face
                            .basis
                            .wake_id
                            .as_ref()
                            .map(|id| digest(id.as_str().as_bytes()))
                            != Some(self.wake)
                        || !status.publisher_retained
                    {
                        return Err(fail(31));
                    }
                    if status.source_sequence == 0 {
                        return Ok(None);
                    }
                    if status.refusal.is_some() {
                        return Err(fail(31));
                    }
                    let show = self.mask.prepare_show(face).map_err(|_| fail(32))?;
                    let output = self
                        .mask
                        .render_prepared(&show, u64::from(self.next_page) * 3_000)
                        .map_err(|_| fail(33))?;
                    Frame::copy(output.canonical)
                        .map(Some)
                        .map_err(|_| fail(34))
                })
                .map_err(|_| fail(35))??;
            if let Some(output) = output {
                self.next_page += 1;
                return Ok(Some(output));
            }
            if Instant::now() >= deadline {
                return Err(fail(36));
            }
            Timer::after(Duration::from_millis(100)).await;
        }
    }
    fn accepted(&mut self, sequence: u64) {
        if sequence == u64::from(self.accepted) {
            self.accepted += 1;
        }
    }
    async fn refused(&mut self, _: u64) -> Result<(), k::Failure> {
        // The retained Look supplies brightness three. A semantic refusal is
        // unexpected and must fail the checkpoint rather than be swallowed.
        Err(fail(37))
    }
}
impl NativeDisplay {
    pub async fn register_check_task(
        &mut self,
        spawner: embassy_executor::Spawner,
        reader: &'static crate::native_face::Reader,
        reservation: &crate::native_owner_reservation::OwnerReservation,
    ) -> Result<(), &'static str> {
        if self.worker.is_some() {
            return Err("display-task-already-registered");
        }
        let mask = self.fit.take().ok_or("display-mask-unavailable")?;
        let fore = FaceFore {
            reader,
            mask,
            body: conduit_core::assigned_plan_payload_digest(reservation.body.as_bytes()),
            wake: conduit_core::assigned_plan_payload_digest(reservation.wake.as_bytes()),
            accepted: 0,
            next_page: 0,
        };
        self.worker = Some(super::task::prepare(
            &mut self.execution,
            spawner,
            Some(fore),
        )?);
        let worker = self.worker.as_ref().expect("registered display checkpoint");
        if embassy_time::with_timeout(Duration::from_secs(2), worker.wait_ready())
            .await
            .is_err()
        {
            let _ = worker.cancel();
            return Err("display-task-ready-timeout");
        }
        if worker.status().phase != "ready-held" {
            return Err("display-task-not-ready");
        }
        self.phase = "task-owned";
        Ok(())
    }
}
fn fail(detail: u16) -> k::Failure {
    k::Failure {
        code: k::FailureCode::InvalidLifecycle,
        detail,
    }
}
