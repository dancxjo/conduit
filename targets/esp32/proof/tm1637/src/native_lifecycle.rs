//! Exact fragment custody at Root. The full Body lifecycle remains with its
//! retained host owner; no native call is released by accepting a start alone.
use super::NativeRoot;
use crate::{native_owner_control as owner, native_owner_receipt as ack};
impl NativeRoot {
    pub async fn apply_owner_lifecycle(
        &mut self,
        token: owner::AuthenticatedRequest<'_>,
    ) -> Result<ack::Receipt, &'static str> {
        let operation = token.operation();
        let request = token.into_request();
        let command = request.transcript().map_err(|_| "command-scope")?;
        // Sender proof was already consumed by authenticate_owner. Retain exact
        // reservation checks without manufacturing a second authenticated token.
        self.owner_reservation
            .as_ref()
            .ok_or("owner-unreserved")?
            .check_request(&request, operation)?;
        #[cfg(feature = "display-ready-root")]
        {
            let state = match operation {
                owner::Operation::Start => {
                    self.display.accept_start()?;
                    ack::State::StartedHeld
                }
                owner::Operation::Release => {
                    self.display.release()?;
                    ack::State::Released
                }
                owner::Operation::Cancel => {
                    self.display.cancel_held().await?;
                    ack::State::Quiesced
                }
                owner::Operation::Retire => {
                    let active = &self
                        .owner_reservation
                        .as_ref()
                        .expect("checked reservation")
                        .active;
                    self.display.retire_held(&mut self.issuer, active)?;
                    ack::State::Retired
                }
                _ => return Err("display-worker-unavailable"),
            };
            return Ok(ack::Receipt {
                command,
                state,
                snapshot: None,
            });
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            let worker = self.worker.as_ref().ok_or("worker-unavailable")?;
            let state = match operation {
                owner::Operation::Start => {
                    worker.accept_start()?;
                    ack::State::StartedHeld
                }
                owner::Operation::Release => {
                    worker.release()?;
                    ack::State::Released
                }
                owner::Operation::Cancel => {
                    worker.abort_preparation();
                    embassy_time::with_timeout(embassy_time::Duration::from_secs(2), async {
                        loop {
                            let status = worker.status();
                            if status.phase == "quiesced"
                                && !status.task_active
                                && !status.cleanup_failure
                                && status.native_lifecycles == Some(["revoked", "revoked"])
                                && status.native_in_flight == Some([0, 0])
                            {
                                return Ok(());
                            }
                            if matches!(
                                status.phase,
                                "failed"
                                    | "drained"
                                    | "cleanup-refused"
                                    | "ownership-refused"
                                    | "retired"
                            ) {
                                return Err("worker-quiescence-refused");
                            }
                            embassy_time::Timer::after_millis(5).await;
                        }
                    })
                    .await
                    .map_err(|_| "worker-quiescence-timeout")??;
                    ack::State::Quiesced
                }
                owner::Operation::Retire => {
                    if self.retired_native.is_some() {
                        return Err("native-already-retired");
                    }
                    self.retired_native = Some(worker.retire()?);
                    self.issuer
                        .finish_native_play(
                            &self
                                .owner_reservation
                                .as_ref()
                                .expect("checked reservation")
                                .active,
                            self.retired_native
                                .as_ref()
                                .expect("returned native custody"),
                        )
                        .map_err(|_| "native-retirement-issuer-scope")?;
                    ack::State::Retired
                }
                _ => return Err("operation-not-installed"),
            };
            Ok(ack::Receipt {
                command,
                state,
                snapshot: None,
            })
        }
    }
    pub fn retired_native_attachments(&self) -> Option<[u64; 2]> {
        self.retired_native
            .as_ref()
            .map(|native| [native.pulse.1.generation, native.wait.1.generation])
    }
    pub fn copy_worker_publication(
        &self,
        output: &mut [u8],
    ) -> Result<Option<crate::native_task::Status>, &'static str> {
        self.worker
            .as_ref()
            .map_or(Ok(None), |worker| worker.copy_publication(output))
    }
    pub fn sign_owner_receipt(&self, receipt: &ack::Receipt) -> [u8; 64] {
        self.admission.sign_owner_receipt(receipt)
    }
}
