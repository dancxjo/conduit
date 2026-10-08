//! Explicit synthetic numerical proof appliance. No pretrained model or voice.
use super::{execution::PreparedExecution, resources::PreparedIngress, storage::StaticStorage};
use core::fmt::Write;
static STORAGE: StaticStorage = StaticStorage::new();
struct Serial;
impl core::fmt::Write for Serial {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        crate::arch::append_boot_diagnostic(text.as_bytes()).map_err(|_| core::fmt::Error)
    }
}
pub fn run(record: &crate::boot::BootRecord, identities: crate::identity::BootIdentities) -> ! {
    let make = &crate::make::EMBEDDED_MAKE;
    crate::arch::early_write(b"CONDUIT_NUMERIC_PROOF_START\n");
    let mut serial = Serial;
    if super::profile::validate_make(make).is_err() {
        let _ =
            crate::arch::append_boot_diagnostic(b"CONDUIT_NUMERIC_PROOF_REFUSED make-profile\n");
        halt();
    }
    let _ = writeln!(
        serial,
        "CONDUIT_NUMERIC_PROOF_BEGIN profile={} arena={} stack={} fixed_store={} boot_timestamp={}",
        super::PREPARATION_PROFILE,
        super::ARENA_BYTES,
        super::PREPARATION_STACK_BYTES,
        core::mem::size_of::<StaticStorage>(),
        record.timestamp
    );
    let Some(materials) = super::embedded::MATERIALS.as_ref() else {
        let _ = crate::arch::append_boot_diagnostic(
            b"CONDUIT_NUMERIC_PROOF_REFUSED missing-local-fixture\n",
        );
        halt();
    };
    let data = super::Materials {
        source: materials.source,
        reference_image: materials.reference_image,
        native_definition: materials.native_definition,
        recipe: materials.recipe,
    };
    let host = crate::identity::hex(&identities.host).into();
    let boot = crate::identity::hex(&identities.boot).into();
    let topology = match super::PreparedTopology::prepare(data, host, boot) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(serial, "CONDUIT_NUMERIC_PROOF_REFUSED preparation={error}");
            halt();
        }
    };
    let _ = writeln!(
        serial,
        "CONDUIT_NUMERIC_PROOF_PLANNED source={} checked={} expanded={} plan={} reference_plan={} host={} boot={} nodes={} cords={} arena_peak={} arena_live={}",
        topology.plan.source_document_id.as_str(),
        topology.plan.checked_plot_id.as_str(),
        topology.plan.expanded_plot_id.as_str(),
        topology.plan.plan_id.as_str(),
        topology.recipe.reference_plan_id.as_str(),
        topology.plan.fragments[0].host_id.as_str(),
        topology.plan.fragments[0].boot_id.as_str(),
        topology.plan.fragments[0].placements.len(),
        topology.plan.fragments[0].connections.len(),
        crate::allocation::BOOT_ARENA.used(),
        crate::allocation::BOOT_ARENA.live_bytes()
    );
    let ingress = match PreparedIngress::prepare(&topology, super::embedded::FILES) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(serial, "CONDUIT_NUMERIC_PROOF_REFUSED ingress={error}");
            halt();
        }
    };
    let _ = writeln!(
        serial,
        "CONDUIT_NUMERIC_PROOF_CUSTODY resources={} raw_bytes={} inline_descriptors={} ingress={}",
        ingress.resources.len(),
        ingress.raw_resource_bytes,
        ingress.descriptor_inline_bytes,
        ingress.values.len()
    );
    let storage = match STORAGE.claim() {
        Ok(value) => value,
        Err(_) => {
            let _ = crate::arch::append_boot_diagnostic(
                b"CONDUIT_NUMERIC_PROOF_REFUSED storage-claimed\n",
            );
            halt();
        }
    };
    let mut execution = match PreparedExecution::prepare(&topology, ingress, storage) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(serial, "CONDUIT_NUMERIC_PROOF_REFUSED owners={error}");
            halt();
        }
    };
    let peak = crate::allocation::BOOT_ARENA.seal();
    let before = crate::allocation::BOOT_ARENA.allocation_requests();
    let live = crate::allocation::BOOT_ARENA.live_bytes();
    let result = execution.run();
    let after = crate::allocation::BOOT_ARENA.allocation_requests();
    let live_after = crate::allocation::BOOT_ARENA.live_bytes();
    let _ = writeln!(
        serial,
        "CONDUIT_NUMERIC_PROOF_RESULT result={result:?} prep_peak={peak} retained_arena={live} retained_arena_after={live_after} allocation_requests_before={} allocation_requests_after={} sealed_requests_before={} sealed_requests_after={} capacity={}",
        before.total,
        after.total,
        before.after_seal,
        after.after_seal,
        crate::allocation::BOOT_ARENA.capacity()
    );
    if result.is_ok() && before == after && after.after_seal == 0 && live == live_after {
        let _ = crate::arch::append_boot_diagnostic(
            b"CONDUIT_NUMERIC_PROOF_PASS synthetic-topology-only\n",
        );
    } else {
        let _ = crate::arch::append_boot_diagnostic(b"CONDUIT_NUMERIC_PROOF_FAILED\n");
    }
    // Retain full preparation receipts, Plan, schemas and source through halt.
    core::hint::black_box(&topology);
    crate::arch::deterministic_exit(
        result.is_ok() && before == after && after.after_seal == 0 && live == live_after,
    )
}
fn halt() -> ! {
    crate::arch::deterministic_exit(false)
}
