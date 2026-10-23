//! CPU fixtures exercise the production Root issuer; no physical identity or
//! hardware entropy is established by these tests.
use super::*;
fn prepare(issuer: &native_authority::RootIssuer) -> network_planning::PreparedNetworkSetup {
    let (source, join) = fixtures::ready(Rc::new(fixtures::State::default()), true);
    let mut host = fixtures::host();
    source.append_to_advertisement(&mut host).unwrap();
    join.append_to_advertisement(&mut host).unwrap();
    native_play::NativePlay::new().append_offer(&mut host);
    let grants = [
        issuer
            .grant(
                source.native_identity(),
                native_authority::Operation::Credentials,
            )
            .unwrap(),
        issuer
            .grant(join.native_identity(), native_authority::Operation::Join)
            .unwrap(),
    ];
    network_planning::PreparedNetworkSetup::prepare(
        &host,
        &["conduit.base/local@1".into()],
        &grants,
    )
    .unwrap()
}
#[test]
fn only_root_reserved_play_identity_can_receive_current_capability_possession() {
    use native_authority::Operation;
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    let plan = prepare(&issuer);
    let fragment = &plan.plan().fragments[0];
    let native = fixtures::identity(true);
    let computed = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    // Correct, computable metadata is not an independently reserved Root Play.
    assert!(issuer
        .issue(&native, Operation::Join, fragment, &computed)
        .is_err());
    let first = issuer.reserve_play(fragment).unwrap();
    assert_eq!(first, computed);
    let unreserved =
        c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    assert!(issuer
        .issue(&native, Operation::Join, fragment, &unreserved)
        .is_err());
    let second = issuer.reserve_play(fragment).unwrap();
    assert_eq!(second, unreserved);
    // A superseded reservation cannot issue even a still-unused operation.
    assert!(issuer
        .issue(&native, Operation::Join, fragment, &first)
        .is_err());
    let (mut table, handle, claim) = issuer
        .issue(&native, Operation::Join, fragment, &second)
        .unwrap();
    assert_eq!(claim.active_play_id, second.active_play_id);
    table.authorize(&handle, &claim).unwrap();
    assert!(issuer
        .issue(&native, Operation::Join, fragment, &second)
        .is_err());
}
#[test]
fn invalid_reservation_preserves_counter_and_existing_reserved_scope() {
    use native_authority::Operation;
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    let plan = prepare(&issuer);
    let fragment = &plan.plan().fragments[0];
    let first = issuer.reserve_play(fragment).unwrap();
    for change in 0..3 {
        let mut invalid = fragment.clone();
        match change {
            0 => invalid.host_id = "host/foreign".into(),
            1 => invalid.boot_id = "boot/stale".into(),
            _ => invalid.plan_id = "plan/forged".into(),
        }
        assert!(issuer.reserve_play(&invalid).is_err());
    }
    let native = fixtures::identity(false);
    issuer
        .issue(&native, Operation::Credentials, fragment, &first)
        .unwrap();
    let next = issuer.reserve_play(fragment).unwrap();
    assert_eq!(next.play_sequence, 2);
    assert_ne!(first.active_play_id, next.active_play_id);
}
