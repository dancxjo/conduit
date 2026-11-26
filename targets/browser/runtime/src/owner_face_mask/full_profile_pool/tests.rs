use super::*;
#[path = "allocation_probe.rs"]
mod allocation_probe;
fn capacities() -> (usize, usize) {
    (
        conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize + 12 * 1024,
        conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES as usize,
    )
}
fn interface() -> (Vec<u8>, Vec<u8>) {
    let (input, output) = capacities();
    admit_bootstrap(input + output + 2 * std::mem::size_of::<Vec<u8>>()).unwrap();
    let buffers = (Vec::with_capacity(input), Vec::with_capacity(output));
    mark_interface_ready(buffers.0.capacity(), buffers.1.capacity()).unwrap();
    buffers
}
fn host(host: HostId, boot: BootId) -> HostAdvertisement {
    let mut ad = crate::installed_browser::membership_advertisement(host, boot);
    ad.capabilities.push(profile::offer());
    ad.resources.push(conduit_core::resource_offer(
        profile::MEMORY_POOL,
        profile::MEMORY_CLASS,
        profile::PREPARATION_BYTES * profile::PREPARATION_SLOTS as u32,
    ));
    ad.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    ad.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    ad
}
fn captured() -> (HostId, BootId, Vec<u8>, Vec<u8>) {
    let h = HostId::from("h".repeat(128));
    let b = BootId::from("b".repeat(128));
    prepare(&h, &b).unwrap();
    let (input, output) = interface();
    let ad = host(h.clone(), b.clone());
    let bytes = serde_json::to_vec(&ad).unwrap();
    capture_advertisement(&ad, &bytes).unwrap();
    freeze().unwrap();
    (h, b, input, output)
}
#[test]
fn compiled_boot_cache_and_real_interfaces_fit_separate_admitted_profile() {
    reset().unwrap();
    let ((h, b, input, output), peak, allocations) = allocation_probe::measure(captured);
    assert!(allocations > 0);
    assert!(
        peak <= BOOTSTRAP_BYTES,
        "bootstrap requested peak {peak} exceeds {BOOTSTRAP_BYTES}"
    );
    let e = snapshot(&h, &b).unwrap();
    assert!(!e.canonical_bytes().is_empty());
    assert_eq!(e.basis().host_id, h);
    assert_eq!(e.basis().boot_id, b);
    assert_eq!(e.offer_generation(), e.basis().offer_generation);
    assert!(e.charged_bytes() + input.capacity() + output.capacity() <= baseline_admitted());
    eprintln!("full Mask Boot CPU profile requested peak={peak}; retained witness bound={}; allocations={allocations}; input={}; output={}",e.charged_bytes(),input.capacity(),output.capacity());
    assert!(reset().is_err());
    drop(e);
    drop(input);
    drop(output);
    reset().unwrap();
}
#[test]
fn last_rc_custody_releases_one_slot_and_refusals_preserve_current() {
    reset().unwrap();
    let (h, b, input, output) = captured();
    let a = Lease::acquire_current().unwrap();
    assert!(a.matches(&h, &b));
    let alias = a.clone();
    let current = Lease::acquire_current().unwrap();
    assert!(Lease::acquire_current().is_err());
    a.admit(profile::PREPARATION_BYTES as usize).unwrap();
    assert!(a.admit(profile::PREPARATION_BYTES as usize + 1).is_err());
    assert_eq!(alias.admitted(), profile::PREPARATION_BYTES as usize);
    drop(a);
    assert!(Lease::acquire_current().is_err());
    assert!(!alias.matches(&HostId::from("foreign"), &b));
    drop(alias);
    let replacement = Lease::acquire_current().unwrap();
    assert!(reset().is_err());
    drop(replacement);
    drop(current);
    drop(input);
    drop(output);
    reset().unwrap();
}
#[test]
fn missing_actual_interfaces_wrong_snapshot_and_incomplete_export_refuse() {
    reset().unwrap();
    let h = HostId::from("host/pool");
    let b = BootId::from("boot/pool");
    prepare(&h, &b).unwrap();
    assert!(!ready(&h, &b));
    assert!(freeze().is_err());
    let (i, o) = capacities();
    assert!(mark_interface_ready(i, o).is_err());
    assert!(admit_bootstrap(BOOTSTRAP_BYTES).is_err());
    let (input, output) = interface();
    let ad = host(h, b);
    assert!(capture_advertisement(&ad, b"{}").is_err());
    assert!(Lease::acquire_current().is_err());
    drop(input);
    drop(output);
    reset().unwrap();
}
