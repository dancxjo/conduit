//! Attended cold qualification, before any local Plot worker starts. This is
//! deliberately not a sampler quiet window or an ordinary timing profile.
use super::NativeRoot;
use crate::native_body_join::membership::job::{Job, Phase};
use crate::native_body_join_uart::{encode_json, write, Bootstrap};
use crate::native_evidence_region::measurement::ReviewedTiming;
use alloc::vec::Vec;
use embassy_time::{with_timeout, Duration, Instant, Timer};
use esp_hal::uart::Uart;

/// Affine exclusive ownership of the actual installed Root before workers.
/// No DTO, caller boolean, or public constructor can create this witness.
pub(crate) struct ColdOwnership<'root> {
    root: &'root mut NativeRoot,
}
impl<'root> ColdOwnership<'root> {
    fn take(root: &'root mut NativeRoot) -> Result<Self, &'static str> {
        let owner = root.body.as_deref().ok_or("cold-membership-body")?;
        let current = owner.evidence().membership.parts.iter().any(|part| {
            part.state == conduit_body::MembershipState::Admitted
                && part.current.as_ref().is_some_and(|current| {
                    current.host_id == root.host && current.boot_id == root.boot
                })
        });
        if !current
            || owner.foreground().is_some()
            || owner.realization().is_some()
            || root.worker.is_some()
            || root.worker_spawner.is_some()
            || root.execution.is_some()
            || root.retired_native.is_some()
            || root.local_mask_worker.is_some()
            || root.local_renewal.is_some()
            || root.membership.is_some()
            || root.observed_offer.is_some()
            || root.observed_reclaim.is_some()
            || root.pending_network_workspace.is_some()
            || !crate::native_restart::publication_ready()
        {
            return Err("cold-membership-not-exclusive");
        }
        root.body_join_scope()?;
        Ok(Self { root })
    }
}
struct Issued {
    claim: conduit_body::SpawnInvitationClaim,
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
}
impl Issued {
    fn check(
        &self,
        root: &NativeRoot,
        request: &conduit_body::PortableMembershipAdmissionRequest,
        now: u64,
    ) -> Result<(), &'static str> {
        let (host, boot, body, _) = root.body_join_scope()?;
        if host != &self.host
            || boot != &self.boot
            || body != &self.claim.body_id
            || request.body_id != self.claim.body_id
            || request.invitation_id != self.claim.invitation_id
            || now >= self.claim.expires_at_millis
        {
            return Err("cold-membership-invitation-scope");
        }
        Ok(())
    }
}
impl NativeRoot {
    /// No request means ordinary startup after two seconds. A challenge can
    /// extend this once; only a signed owner's issued invitation extends further.
    pub(crate) async fn qualify_cold_membership(
        &mut self,
        uart: &mut Uart<'_, esp_hal::Async>,
        timing: Option<ReviewedTiming>,
    ) -> Result<(), &'static str> {
        let timing = timing.ok_or("cold-membership-flash-timing")?;
        let mut scope = ColdOwnership::take(self)?;
        let mut bootstrap = Bootstrap::configured()?;
        let mut deadline = Instant::now().as_millis() + 2_000;
        let mut challenged = false;
        let mut issued: Option<Issued> = None;
        let mut command = Vec::new();
        let mut began = 0;
        let mut oversized = false;
        loop {
            let now = Instant::now().as_millis();
            if now >= deadline {
                command.fill(0);
                return Ok(());
            }
            if !command.is_empty() && now.saturating_sub(began) >= 5_000 {
                command.fill(0);
                command.clear();
            }
            let mut byte = [0];
            match with_timeout(Duration::from_millis(20), uart.read_async(&mut byte)).await {
                Err(_) => continue,
                Ok(Err(_)) => return Err("cold-membership-uart-read"),
                Ok(Ok(0)) => continue,
                _ => {}
            }
            if oversized {
                if byte[0] == b'\n' {
                    oversized = false;
                }
                continue;
            }
            if command.is_empty() {
                if byte[0] != b'J' {
                    continue;
                }
                command
                    .try_reserve_exact(1024)
                    .map_err(|_| "cold-membership-command-memory")?;
                began = now;
            }
            if byte[0] == b'\r' {
                continue;
            }
            if byte[0] != b'\n' {
                if command.len() == 1024 {
                    command.fill(0);
                    command.clear();
                    oversized = true;
                    continue;
                }
                command.push(byte[0]);
                continue;
            }
            if let Some(payload) = command.strip_prefix(b"JOIN! ") {
                let request = match crate::native_body_join::decode_command(payload) {
                    Ok(crate::native_body_join::Command::Membership(request)) => request,
                    _ => {
                        command.fill(0);
                        command.clear();
                        write(uart, b"\n{\"event\":\"cold-membership-refused\"}\n").await?;
                        continue;
                    }
                };
                let valid = issued
                    .as_ref()
                    .ok_or("cold-membership-owner-required")
                    .and_then(|i| i.check(scope.root, &request, now));
                command.fill(0);
                command.clear();
                if valid.is_err() {
                    write(uart, b"\n{\"event\":\"cold-membership-refused\"}\n").await?;
                    continue;
                }
                return run(&mut scope, uart, timing, request).await;
            }
            // After minting, never replace the invitation this window accepts.
            if issued.is_some() {
                command.fill(0);
                command.clear();
                continue;
            }
            let challenge = command == b"JOIN?";
            let mut minted = None;
            let result =
                bootstrap.cold_reply(scope.root, core::mem::take(&mut command), |invitation| {
                    minted = Some(Issued {
                        claim: invitation.invitation.claim.clone(),
                        host: invitation.producer_host.clone(),
                        boot: invitation.producer_boot.clone(),
                    });
                });
            if let Some(invitation) = minted {
                deadline = invitation.claim.expires_at_millis;
                issued = Some(invitation);
            }
            match result {
                Ok(reply) => {
                    if challenge && !challenged {
                        deadline = now + 30_000;
                        challenged = true;
                    }
                    write(uart, b"\n").await?;
                    write(uart, &reply.0).await?;
                    write(uart, b"\n").await?;
                }
                Err(_) => write(uart, b"\n{\"event\":\"cold-membership-refused\"}\n").await?,
            }
        }
    }
}
async fn retain(scope: &ColdOwnership<'_>, job: &Job, reason: &'static str) -> ! {
    esp_println::println!("{{\"event\":\"cold-membership-retained-failure\",\"reason\":\"{}\",\"local_startup_blocked\":true}}", reason);
    loop {
        core::hint::black_box((scope, job));
        Timer::after_secs(60).await;
    }
}
async fn run(
    scope: &mut ColdOwnership<'_>,
    uart: &mut Uart<'_, esp_hal::Async>,
    timing: ReviewedTiming,
    request: conduit_body::PortableMembershipAdmissionRequest,
) -> Result<(), &'static str> {
    let owner = scope.root.body.take().ok_or("cold-membership-body")?;
    let mut job = match Job::reserve(
        owner,
        scope.root.admission.verifying_key(),
        scope.root.host.clone(),
        scope.root.boot.clone(),
        request,
    ) {
        Ok(job) => job,
        Err((owner, _, _, _, reason)) => {
            scope.root.body = Some(owner);
            return Err(reason);
        }
    };
    loop {
        let phase = job.phase();
        if phase == Phase::Checkpoint(crate::native_restart::owned_checkpoint::Phase::Complete) {
            break;
        }
        let before = esp_alloc::HEAP.stats();
        let began = Instant::now().as_micros();
        let result = job.advance_cold(
            scope,
            &mut crate::native_evidence_region::checkpoint_access::cold::Access::new(scope, timing),
            Instant::now().as_millis(),
        );
        let elapsed = Instant::now().as_micros().saturating_sub(began);
        let after = esp_alloc::HEAP.stats();
        esp_println::println!("{{\"event\":\"cold-membership-phase\",\"phase\":\"{:?}\",\"elapsed_us\":{},\"cache_bytes\":{},\"heap_before\":{},\"heap_after\":{},\"boot_heap_peak\":{},\"heap_scope\":\"root_allocator\",\"phase_peak_measured\":false,\"total_ram_measured\":false,\"during_local_play\":false,\"timing_qualified\":false}}", phase, elapsed, job.cached_bytes(), before.current_usage, after.current_usage, after.max_usage);
        if let Err(reason) = result {
            retain(scope, &job, reason).await;
        }
        // Yield without granting a fictional sampler or changing phase budgets.
        Timer::after_millis(1).await;
    }
    let (owner, admitted) = match job.finish() {
        Ok(done) => done,
        Err(job) => retain(scope, &job, "cold-membership-adopt").await,
    };
    scope.root.body = Some(owner);
    let signature = scope.root.admission.sign_membership(&admitted)?;
    let receipt = admitted.signed(signature);
    let reply = encode_json(&receipt)?;
    write(uart, b"\n").await?;
    write(uart, &reply.0).await?;
    write(uart, b"\n").await?;
    Ok(())
}
