use super::*;
const CONTRACT: &str = "fixture/host-command-json@1";
const PAYLOAD: &[u8] = b"immutable-input";

fn stamp(f: &Fixture) -> RetainedHostCommandStamp {
    let slot = f.slot([3; 32]);
    let signature = f
        .key
        .sign(&host_command_attribution_transcript(slot.challenge(), PAYLOAD).unwrap())
        .to_bytes();
    slot.authenticate(PAYLOAD, &signature, 11)
        .unwrap()
        .into_retained_stamp()
        .unwrap()
}
fn held(f: &Fixture) -> HostCommandInput<64> {
    let mut input = HostCommandInput::new();
    input.writable().unwrap()[..PAYLOAD.len()].copy_from_slice(PAYLOAD);
    input.retain(stamp(f)).unwrap();
    input
}

#[test]
fn same_receive_buffer_retains_original_attribution_without_allocating() {
    let f = Fixture::new();
    let mut input = HostCommandInput::<64>::new();
    input.writable().unwrap()[..PAYLOAD.len()].copy_from_slice(PAYLOAD);
    let address = input.writable().unwrap().as_ptr();
    let ((), cost) = probe::observe(|| {
        input.retain(stamp(&f)).unwrap();
        assert_eq!(input.payload().unwrap(), PAYLOAD);
        assert_eq!(input.payload().unwrap().as_ptr(), address);
        assert_eq!(
            input.writable().err(),
            Some(HostCommandRetentionRefusal::Occupied)
        );
        let command = input
            .reborrow(&f.member, &f.host, &f.issuer, CONTRACT, 19)
            .unwrap();
        assert_eq!(command.challenge().nonce, [3; 32]);
        assert_eq!(command.challenge().issued_at_millis, 10);
        assert_eq!(command.challenge().expires_at_millis, 20);
        assert_eq!(command.payload().as_ptr(), address);
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(core::mem::size_of::<RetainedHostCommandStamp>() <= 128);
    assert_eq!(
        input.retain(stamp(&f)).err(),
        Some(HostCommandRetentionRefusal::Occupied)
    );
    input.retire();
    assert!(!input.is_retained());
    assert!(input.writable().unwrap().iter().all(|b| *b == 0));
    assert_eq!(
        input
            .reborrow(&f.member, &f.host, &f.issuer, CONTRACT, 19)
            .err(),
        Some(HostCommandRetentionRefusal::NotRetained)
    );
}

#[test]
fn retained_input_never_renews_expiry_or_accepts_another_current_context() {
    for case in 0..14 {
        let mut f = Fixture::new();
        let input = held(&f);
        let mut contract = CONTRACT;
        match case {
            0 => f.host.host_id = "host/other".into(),
            1 => f.host.boot_id = "boot/other".into(),
            2 => f.host.offer_generation.0 += 1,
            3 => f.host.profile = "profile/other".into(),
            4 => {
                f.member.body_id = serde_json::from_value(serde_json::json!("body/other")).unwrap()
            }
            5 => {
                f.member.part_id = serde_json::from_value(serde_json::json!("part/other")).unwrap()
            }
            6 => {
                f.member.credential_id =
                    serde_json::from_value(serde_json::json!("credential/other")).unwrap()
            }
            7 => f.issuer.host_id = "issuer/other".into(),
            8 => f.issuer.boot_id = "issuer/boot/other".into(),
            9 => f.issuer.offer_generation.0 += 1,
            10 => f.issuer.verifying_key = [4; 32],
            11 => contract = "fixture/another-contract@1",
            12 => f.member.host_id = "member/other".into(),
            _ => f.member.boot_id = "member/boot/other".into(),
        }
        assert_eq!(
            input
                .reborrow(&f.member, &f.host, &f.issuer, contract, 19)
                .err(),
            Some(HostCommandRetentionRefusal::Context),
            "case {case}"
        );
        assert_eq!(input.payload().unwrap(), PAYLOAD);
    }
    let f = Fixture::new();
    let input = held(&f);
    for now in [9, 20, u64::MAX] {
        assert_eq!(
            input
                .reborrow(&f.member, &f.host, &f.issuer, CONTRACT, now)
                .err(),
            Some(HostCommandRetentionRefusal::Clock)
        );
    }
}

#[test]
fn substituted_or_undersized_storage_refuses_without_sealing_or_allocating() {
    let f = Fixture::new();
    let mut changed = HostCommandInput::<64>::new();
    changed.writable().unwrap()[..PAYLOAD.len()].copy_from_slice(PAYLOAD);
    changed.writable().unwrap()[0] ^= 1;
    let original = *changed.writable().unwrap();
    let (result, cost) = probe::observe(|| changed.retain(stamp(&f)));
    assert_eq!(result, Err(HostCommandRetentionRefusal::Payload));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(*changed.writable().unwrap(), original);
    let mut small = HostCommandInput::<1>::new();
    assert_eq!(
        small.retain(stamp(&f)),
        Err(HostCommandRetentionRefusal::Bounds)
    );
}

#[test]
fn admitted_attempt_stamp_rejects_fresh_valid_commands_even_for_identical_bytes() {
    let f = Fixture::new();
    let original = stamp(&f);
    let input = held(&f);
    let same = input
        .reborrow(&f.member, &f.host, &f.issuer, CONTRACT, 19)
        .unwrap();
    let (result, cost) = probe::observe(|| original.check_command(&same));
    assert_eq!(result, Ok(()));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    for (nonce, payload) in [([4; 32], PAYLOAD), ([3; 32], &b"different-input"[..])] {
        let slot = f.slot(nonce);
        let signature = f
            .key
            .sign(&host_command_attribution_transcript(slot.challenge(), payload).unwrap())
            .to_bytes();
        let replacement = slot.authenticate(payload, &signature, 11).unwrap();
        let (result, cost) = probe::observe(|| original.check_command(&replacement));
        assert_eq!(result, Err(HostCommandRetentionRefusal::Context));
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    }
    assert_eq!(input.payload(), Some(PAYLOAD));
    assert!(input.is_retained());
}
