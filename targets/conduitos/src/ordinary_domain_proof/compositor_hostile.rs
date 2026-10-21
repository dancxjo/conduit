//! Actual broker negatives using separate hostile ELFs, never the product guest.
use super::refuse;
use crate::{
    arch,
    protected_compositor::{CompositorService, TransportError},
    service_memory::ServiceImage,
    sign_format::FixedText,
};
use conduitos_compositor_core::wire::{Request, Response};
use core::fmt::Write;
use sha2::{Digest, Sha256};
const CASES: [(&str, &[u8], TransportError, u64); 4] = [
    (
        "envelope",
        include_bytes!(concat!(env!("OUT_DIR"), "/compositor-proof-envelope.elf")),
        TransportError::Protocol,
        0,
    ),
    (
        "shape",
        include_bytes!(concat!(env!("OUT_DIR"), "/compositor-proof-shape.elf")),
        TransportError::Protocol,
        0,
    ),
    (
        "fault",
        include_bytes!(concat!(env!("OUT_DIR"), "/compositor-proof-fault.elf")),
        TransportError::Terminated,
        0,
    ),
    (
        "loop",
        include_bytes!(concat!(env!("OUT_DIR"), "/compositor-proof-loop.elf")),
        TransportError::Terminated,
        1,
    ),
];
fn read(address: *const u64) -> u64 {
    unsafe { address.read_volatile() }
}
pub(super) fn run() {
    let root = 0x57a2_bc19_6d40_e38fu64;
    let root_address = &root as *const u64;
    let mut sibling =
        arch::TextDomain::install().unwrap_or_else(|_| refuse("compositor-hostile-sibling"));
    sibling
        .input(b"broker-sibling-sentinel")
        .unwrap_or_else(|_| refuse("compositor-hostile-sibling-input"));
    let sibling_address = sibling.private_frame_address() as *const u64;
    let root_before = read(root_address);
    let sibling_before = read(sibling_address);
    let mut sign = FixedText::new();
    write!(sign,"CONDUIT_COMPOSITOR_HOSTILE_PROOF {{\"schema\":\"conduit.conduitos/compositor-hostile-broker-fixture@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"production_guest_modified\":false,\"direct_memory_attack\":false,\"semantic_show_acknowledged\":false,\"physical_scanout\":false,\"cases\":[").unwrap_or_else(|_|refuse("compositor-hostile-sign"));
    for (index, (name, bytes, expected, preemptions)) in CASES.into_iter().enumerate() {
        let image =
            ServiceImage::compositor(bytes).unwrap_or_else(|_| refuse("compositor-hostile-image"));
        let mut hostile = CompositorService::install(&image, 32, 24)
            .unwrap_or_else(|_| refuse("compositor-hostile-init"));
        let instance = hostile.instance();
        if hostile.request(Request::DamagePending) != Err(expected) {
            refuse("compositor-hostile-outcome");
        }
        let cost = hostile.cost();
        if cost.entries != 2
            || cost.address_space_switches != 4
            || cost.scheduler_returns != 2
            || cost.preemptions != preemptions
            || cost.teardown_zeroed_bytes == 0
            || read(root_address) != root_before
            || read(sibling_address) != sibling_before
        {
            refuse("compositor-hostile-cost-or-sentinel");
        }
        // Keep the inert hostile client alive while installing the unchanged
        // production ELF. These checks preserve state; they are not attacks on
        // the sentinel addresses and do not independently prove memory isolation.
        let mut fresh = CompositorService::install_embedded(32, 24)
            .unwrap_or_else(|_| refuse("compositor-hostile-replacement"));
        if fresh.instance() <= instance
            || hostile.request(Request::DamagePending) != Err(TransportError::Terminated)
            || hostile.upload_chunk(0, &[1]) != Err(TransportError::Terminated)
            || hostile.cost() != cost
        {
            refuse("compositor-hostile-stale-client");
        }
        drop(hostile);
        if fresh.request(Request::DamagePending) != Ok(Response::DamagePending(false))
            || read(root_address) != root_before
            || read(sibling_address) != sibling_before
        {
            refuse("compositor-hostile-fresh-state");
        }
        drop(fresh);
        if index > 0 {
            write!(sign, ",").unwrap_or_else(|_| refuse("compositor-hostile-sign"));
        }
        write!(sign,"{{\"case\":\"{name}\",\"error\":\"{}\",\"entries\":2,\"preemptions\":{preemptions},\"root_sentinel_preserved\":true,\"sibling_sentinel_preserved\":true,\"fresh_production_guest\":true,\"image_sha256\":\"",if expected==TransportError::Protocol {"protocol"} else {"terminated"}).unwrap_or_else(|_|refuse("compositor-hostile-sign"));
        for byte in Sha256::digest(bytes) {
            write!(sign, "{byte:02x}").unwrap_or_else(|_| refuse("compositor-hostile-hash"));
        }
        write!(sign, "\"}}").unwrap_or_else(|_| refuse("compositor-hostile-sign"));
    }
    writeln!(sign, "]}}").unwrap_or_else(|_| refuse("compositor-hostile-sign"));
    arch::early_write(sign.as_bytes());
}
