//! Real authentication and reservation checks across changed offer availability.
use crate::{
    allocation_probe, native_owner_control as c, native_owner_reservation::OwnerReservation,
};
use conduit_core::PlanFragment;
use ed25519_dalek::{Signer, SigningKey};
pub(super) fn check(reserved: &OwnerReservation, fragment: &PlanFragment) {
    let key = SigningKey::from_bytes(&[11; 32]);
    let mut gate = c::OwnerControl::new(Some(key.verifying_key().to_bytes())).unwrap();
    let current = reserved.offer_generation + 2;
    let make = |sequence, operation, offer_generation| c::Request {
        protocol: c::PROTOCOL,
        sequence,
        operation,
        offer_generation,
        host: fragment.host_id.as_str(),
        boot: fragment.boot_id.as_str(),
        body: &reserved.body,
        wake: &reserved.wake,
        body_plan: &reserved.body_plan,
        body_play: &reserved.body_play,
        plan: reserved.active.plan_id.as_str(),
        fragment: reserved.fragment.as_str(),
        fragment_play: reserved.active.active_play_id.as_str(),
        assignment_seal: reserved.assignment_seal,
    };
    for (index, operation) in [
        c::Operation::Prepare,
        c::Operation::Start,
        c::Operation::Release,
        c::Operation::Cancel,
        c::Operation::Retire,
        c::Operation::Inspect,
    ]
    .into_iter()
    .enumerate()
    {
        let (result, memory) = allocation_probe::observe(|| {
            let token = authenticate(
                &key,
                fragment,
                current,
                &mut gate,
                make(index as u64 + 1, operation, reserved.offer_generation),
                Some(reserved),
            )
            .unwrap();
            if operation == c::Operation::Inspect {
                assert!(OwnerReservation::inspection_command(Some(reserved), token)
                    .unwrap()
                    .is_some());
                Ok(())
            } else {
                reserved.authorize_operation(token, operation)
            }
        });
        assert_eq!(result, Ok(()));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    // A valid owner signature cannot relabel this worker's committed value.
    for change in 0..8 {
        let mut request = make(
            100 + change,
            c::Operation::Inspect,
            reserved.offer_generation,
        );
        match change {
            0 => request.body = "other/body",
            1 => request.wake = "other/wake",
            2 => request.body_plan = "other/body-plan",
            3 => request.body_play = "other/body-play",
            4 => request.plan = "other/plan",
            5 => request.fragment = "other/fragment",
            6 => request.fragment_play = "other/play",
            _ => request.assignment_seal[0] ^= 1,
        }
        let token =
            authenticate(&key, fragment, current, &mut gate, request, Some(reserved)).unwrap();
        assert_eq!(
            OwnerReservation::inspection_command(Some(reserved), token),
            Err("reservation-scope")
        );
    }
    let before = gate.last_sequence();
    for operation in [c::Operation::Cancel, c::Operation::Inspect] {
        let request = make(200, operation, current);
        assert!(matches!(
            authenticate(&key, fragment, current, &mut gate, request, Some(reserved)),
            Err(c::Refusal::WrongGeneration)
        ));
        assert_eq!(gate.last_sequence(), before);
    }
    let mut stale = make(200, c::Operation::Cancel, reserved.offer_generation);
    stale.boot = "stale/boot";
    assert!(matches!(
        authenticate(&key, fragment, current, &mut gate, stale, Some(reserved)),
        Err(c::Refusal::StaleBoot)
    ));
    assert_eq!(gate.last_sequence(), before);
    // No reservation: authenticated metadata inspection yields no publication digest.
    let token = authenticate(
        &key,
        fragment,
        current,
        &mut gate,
        make(201, c::Operation::Inspect, current),
        None,
    )
    .unwrap();
    assert_eq!(OwnerReservation::inspection_command(None, token), Ok(None));
    // New admission never borrows an earlier reservation's offer generation.
    assert!(matches!(
        authenticate(
            &key,
            fragment,
            current,
            &mut gate,
            make(202, c::Operation::Reserve, reserved.offer_generation),
            Some(reserved)
        ),
        Err(c::Refusal::WrongGeneration)
    ));
    let token = authenticate(
        &key,
        fragment,
        current,
        &mut gate,
        make(202, c::Operation::Reserve, current),
        Some(reserved),
    )
    .unwrap();
    assert_eq!(
        OwnerReservation::inspection_command(None, token),
        Err("wrong-operation")
    );
}

fn authenticate<'a>(
    key: &SigningKey,
    fragment: &PlanFragment,
    current: u64,
    gate: &mut c::OwnerControl,
    request: c::Request<'a>,
    reservation: Option<&OwnerReservation>,
) -> Result<c::AuthenticatedRequest<'a>, c::Refusal> {
    let signature = key.sign(&request.transcript().unwrap()).to_bytes();
    let generation =
        OwnerReservation::authentication_generation(request.operation, current, reservation);
    gate.authenticate(
        request,
        &signature,
        fragment.host_id.as_str(),
        fragment.boot_id.as_str(),
        generation,
    )
}
