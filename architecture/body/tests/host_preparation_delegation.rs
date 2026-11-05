#![cfg(feature = "authenticated-admission")]
use conduit_body::*;
#[path = "host_preparation_delegation/support.rs"]
mod support;
use support::{policy, Fixture};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

#[test]
fn valid_membership_and_attribution_need_explicit_installed_delegation() {
    let f = Fixture::new();
    let command = f.command(HOST_PREPARED_FRAGMENT_CONTRACT, b"exact assignment bytes");
    let ((), cost) = probe::observe(|| {
        assert_eq!(
            check_host_preparation_delegation(None, f.context(), &command),
            Err(PreparationDelegationRefusal::MissingDelegation)
        );
        assert_eq!(
            check_host_preparation_delegation(Some(&policy()), f.context(), &command),
            Ok(())
        );
        assert_eq!(command.payload(), b"exact assignment bytes");
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    // Success is a read-only policy check. No capability/resource owner or
    // reservation is supplied to this API, and no kernel/effect can start.
}

#[test]
fn current_membership_receiver_and_exact_seed_issuer_are_rechecked_after_attribution() {
    use PreparationDelegationRefusal as E;
    let source = Fixture::new();
    let command = source.command(HOST_PREPARED_FRAGMENT_CONTRACT, b"exact");
    for change in 0..12 {
        let mut current = Fixture::new();
        let expected = match change {
            0 => {
                current.member.body_id =
                    serde_json::from_value(serde_json::json!("body/other")).unwrap();
                E::Membership
            }
            1 => {
                current.member.part_id =
                    serde_json::from_value(serde_json::json!("part/other")).unwrap();
                E::Membership
            }
            2 => {
                current.member.credential_id =
                    serde_json::from_value(serde_json::json!("credential/new")).unwrap();
                E::Membership
            }
            3 => {
                current.host.boot_id = "boot/new".into();
                current.member.boot_id = current.host.boot_id.clone();
                E::Receiver
            }
            4 => {
                current.host.offer_generation.0 += 1;
                E::Receiver
            }
            5 => {
                current.host.profile = "profile/new".into();
                E::Receiver
            }
            6 => {
                current.bootstrap.owner.host_id = "issuer/other".into();
                E::Issuer
            }
            7 => {
                current.bootstrap.owner.boot_id = "issuer/boot/new".into();
                E::Issuer
            }
            8 => {
                current.bootstrap.owner.offer_generation.0 += 1;
                E::Issuer
            }
            9 => {
                current.bootstrap.owner.verifying_key =
                    ed25519_dalek::SigningKey::from_bytes(&[8; 32])
                        .verifying_key()
                        .to_bytes();
                E::Issuer
            }
            10 => {
                current.host.offer_generation.0 = 0;
                E::Receiver
            }
            _ => {
                current.bootstrap.configuration_id = "config/other".into();
                E::Construction
            }
        };
        let (result, cost) = probe::observe(|| {
            check_host_preparation_delegation(Some(&policy()), current.context(), &command)
        });
        assert_eq!(result, Err(expected), "case {change}");
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
    }
}

#[test]
fn fresh_but_wrongly_scoped_signed_requests_and_invalid_policy_bounds_refuse() {
    use PreparationDelegationRefusal as E;
    let receiver = Fixture::new();
    let wrong_contract = receiver.command("fixture/another-contract@1", b"exact");
    assert_eq!(
        check_host_preparation_delegation(Some(&policy()), receiver.context(), &wrong_contract),
        Err(E::Contract)
    );
    let mut other = Fixture::new();
    other.key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    other.issuer.verifying_key = other.key.verifying_key().to_bytes();
    let command = other.command(HOST_PREPARED_FRAGMENT_CONTRACT, b"exact");
    assert_eq!(
        check_host_preparation_delegation(Some(&policy()), receiver.context(), &command),
        Err(E::Issuer)
    );
    let command = receiver.command(HOST_PREPARED_FRAGMENT_CONTRACT, b"exact");
    for bad in [
        HostPreparationDelegation {
            maximum_assignment_bytes: 0,
            ..policy()
        },
        HostPreparationDelegation {
            maximum_assignment_bytes: 4,
            ..policy()
        },
        HostPreparationDelegation {
            maximum_runtime_state_bytes: 0,
            ..policy()
        },
    ] {
        assert_eq!(
            check_host_preparation_delegation(Some(&bad), receiver.context(), &command),
            Err(E::Bounds)
        );
    }
    for now in [9, 20, u64::MAX] {
        let mut context = receiver.context();
        context.now_millis = now;
        assert_eq!(
            check_host_preparation_delegation(Some(&policy()), context, &command),
            Err(E::Clock)
        );
    }
    let mut context = receiver.context();
    context.image_id = "image/other";
    assert_eq!(
        check_host_preparation_delegation(Some(&policy()), context, &command),
        Err(E::Construction)
    );
}
