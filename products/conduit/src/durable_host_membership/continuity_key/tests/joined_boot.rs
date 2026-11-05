//! An installed member boots without acquiring local Body ownership.
use super::*;

#[test]
fn joined_host_boot_does_not_probe_local_owner_source() {
    let f = fixture();
    complete(&f).unwrap();
    let before = fs::read(f.root.join("installation.json")).unwrap();
    assert!(durable_host::owner::retained_thermostat_scan(&f.root).is_err());
    let (status, runtime) = durable_host::prepare_runtime(&f.root).unwrap();
    assert_eq!(status.host_id, f.receipt.credential.host_id.as_str());
    assert_eq!(
        status.body_id.as_deref(),
        Some(f.receipt.credential.body_id.as_str())
    );
    assert_eq!(
        runtime.truth().advertisement.host_id,
        f.receipt.credential.host_id
    );
    assert_eq!(fs::read(f.root.join("installation.json")).unwrap(), before);
}
