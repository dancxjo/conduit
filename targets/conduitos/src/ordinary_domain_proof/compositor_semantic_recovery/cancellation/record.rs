//! Fixed records for the separate stopped-cancellation fixture, not v1 product checkpoints.
use super::*;
use serde::Serialize;
const PREFIX: &[u8] = b"CONDUIT_COMPOSITOR_STOPPED_LIFECYCLE ";
#[derive(Serialize)]
struct Record<'a> {
    schema: &'static str,
    part: u8,
    phase: &'static str,
    identity: Identity<'a>,
    instance: u64,
    namespace: Option<u64>,
    ack: Option<AckRef>,
    cost: Cost<'a>,
    raw_ticks: Option<u64>,
    staging_live_bytes: usize,
    staging_prepared_bytes: usize,
}
pub(super) fn emit(
    grant: &SharedDisplayGrant,
    snapshots: [DisplayObservation; 6],
    ack: AckRef,
    fresh_ack: AckRef,
    zero: zero_record::Audit,
) {
    let [
        prepared,
        acknowledged,
        before,
        disposed,
        restarted,
        reacknowledged,
    ] = snapshots;
    let disposal = disposed
        .service
        .disposal
        .unwrap_or_else(|| refuse("cancel-disposal-sample"));
    let before_disposal = disposal
        .before
        .unwrap_or_else(|| refuse("cancel-disposal-before"));
    let after_disposal = disposal
        .after
        .unwrap_or_else(|| refuse("cancel-disposal-after"));
    if before.current_cost != before_disposal
        || disposed.current_cost != after_disposal
        || !disposal.succeeded
    {
        refuse("cancel-disposal-cost-binding");
    }
    let phases = [
        (
            "cold-installed",
            prepared,
            prepared.service.installed,
            prepared.service.installation_raw_ticks,
            None,
        ),
        (
            "cold-prepared",
            prepared,
            prepared.current_cost,
            prepared.preparation_raw_ticks,
            None,
        ),
        (
            "initial-ack",
            acknowledged,
            acknowledged.current_cost,
            acknowledged.service.requests.last.and_then(|s| s.raw_ticks),
            Some(ack),
        ),
        ("cancel-before", before, before_disposal, None, None),
        (
            "cancel-service-disposed",
            disposed,
            after_disposal,
            disposal.raw_ticks,
            None,
        ),
        (
            "cancel-owner-disposed",
            disposed,
            after_disposal,
            disposed.disposal_raw_ticks,
            None,
        ),
        (
            "restart-prepared",
            restarted,
            restarted.current_cost,
            restarted.preparation_raw_ticks,
            None,
        ),
        (
            "restart-ack",
            reacknowledged,
            reacknowledged.current_cost,
            reacknowledged
                .service
                .requests
                .last
                .and_then(|s| s.raw_ticks),
            Some(fresh_ack),
        ),
    ];
    let mut buffer = [0_u8; 4096];
    for (part, (phase, observed, cost, ticks, ack)) in phases.into_iter().enumerate() {
        let record = Record {
            schema: "conduit.conduitos/compositor-stopped-lifecycle@1",
            part: part as u8,
            phase,
            identity: Identity::borrowed(
                grant.identity(),
                &crate::make::EMBEDDED_MAKE,
                &prepared.guest_image_sha256,
            ),
            instance: observed.service.instance,
            namespace: ack.map(|value| value.namespace),
            ack,
            cost: Cost(&cost),
            raw_ticks: ticks,
            staging_live_bytes: observed.staging_live_bytes,
            staging_prepared_bytes: observed.staging_prepared_bytes,
        };
        let length = serde_json_core::to_slice(&record, &mut buffer)
            .unwrap_or_else(|_| refuse("cancel-record-bound"));
        if length + PREFIX.len() + 1 > buffer.len() {
            refuse("cancel-record-line-bound");
        }
        arch::early_write(PREFIX);
        arch::early_write(&buffer[..length]);
        arch::early_write(b"\n");
    }
    zero_record::emit(grant, &prepared.guest_image_sha256, ack, zero);
    arch::early_write(b"CONDUIT_COMPOSITOR_STOPPED_LIFECYCLE_RESULT {\"schema\":\"conduit.conduitos/compositor-stopped-lifecycle-result@2\",\"status\":\"completed\",\"stopped_owner_cancellation\":true,\"unfinished_upload_fenced\":true,\"staging_released\":true,\"old_clients_refused\":true,\"stale_drop_preserves_replacement\":true,\"same_retained_grant\":true,\"fresh_semantic_show_and_progress\":true,\"whole_zero_audited\":true,\"asynchronous_cancel\":false,\"guest_cpu_fault\":false,\"malformed_reply\":false,\"physical_scanout\":false}\n");
}
