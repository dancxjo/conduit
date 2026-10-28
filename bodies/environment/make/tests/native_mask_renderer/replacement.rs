//! Actual wire engine and capability tables; deterministic pins, no HIL claim.
use super::*;
use conduit_plan_lowering::selected_operation::*;
#[test]
fn successor_ack_replaces_without_blanking_and_keeps_exact_cleanup_budget() {
    for nack in [false, true] {
        let pins = Rc::new(native::Pins::default());
        let ready = native::ready(pins.clone());
        let planned = planned(&ready);
        let context = f::PreparedMaskShowContext::from_wire(
            serde_json::to_vec(&planned).unwrap().into_boxed_slice(),
            8192,
            65536,
        )
        .unwrap();
        let mut metadata = planned.plan.clone();
        let fragment_id = metadata.fragments[0].fragment_id.clone();
        let fore = metadata.fragments[0].fore_ports.clone();
        let configurations: std::collections::BTreeMap<_, _> = metadata
            .fragments
            .iter_mut()
            .flat_map(|fragment| &mut fragment.placements)
            .map(|gear| {
                (
                    gear.placement_id.clone(),
                    core::mem::take(&mut gear.configuration),
                )
            })
            .collect();
        let verified = c::verify_plan_metadata_with_configurations(metadata, |gear| {
            Ok::<_, ()>(configurations[&gear.placement_id].clone())
        })
        .unwrap();
        let lowered = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
            &verified,
            &fragment_id,
        )
        .unwrap();
        let first = c::bind_active_play(
            verified.plan_id(),
            &ready.identity().host_id,
            &ready.identity().boot_id,
            1,
        );
        let second = c::bind_active_play(
            verified.plan_id(),
            &ready.identity().host_id,
            &ready.identity().boot_id,
            2,
        );
        let (table, handle, claim) = native::issue(&ready, &first);
        let next_lease = native::issue(&ready, &second);
        let wrong_lease = native::issue(&ready, &first);
        let gear = planned.show_placement();
        let mut capsule = ready
            .prepare_verified_invocation(
                &table,
                &claim,
                &gear.configuration,
                SelectedVerifiedForeOperationPlan {
                    fore: &fore,
                    operation: SelectedVerifiedOperationPlan {
                        plan: &verified,
                        fragment_id: &fragment_id,
                        lowered: &lowered,
                        active: &first,
                        placement_id: &gear.placement_id,
                    },
                },
            )
            .unwrap()
            .into_execution();
        let mut owner = Some(ready);
        let mut call = capsule
            .bind_retained(&mut owner, table, handle, claim, &first)
            .unwrap();
        let face = fixture::face();
        let face_wire = serde_json::to_vec(&face).unwrap();
        let basis = |elapsed| PresentationBasis {
            face: &face,
            face_wire: &face_wire,
            context: &context,
            elapsed_ms: elapsed,
            front: "environment".into(),
            target: "fixture/display".into(),
            prepared_sign: "fixture/prepared".into(),
            available_sign: "fixture/ack".into(),
        };
        let predecessor = call.prepare_presentation(basis(0)).unwrap();
        let mut output = [0; 8192];
        call.present_encoded(
            capsule.node(),
            HostCallId(0),
            RequestId(0),
            &face_wire,
            &predecessor,
            &mut output,
            encode,
            || false,
        )
        .unwrap();
        let mut predecessor = call
            .retain_presentation(predecessor)
            .unwrap_or_else(|_| panic!("acknowledged presentation refused"));
        let first_edges = pins.edges.get();
        let (wrong_table, wrong_handle, wrong_claim) = wrong_lease;
        assert!(capsule
            .stage_replacement(
                &call,
                (first.clone(), wrong_table, wrong_handle, wrong_claim)
            )
            .is_err());
        assert_eq!(pins.edges.get(), first_edges);
        let (table, handle, claim) = next_lease;
        let mut successor = capsule
            .stage_replacement(&call, (second.clone(), table, handle, claim))
            .unwrap_or_else(|_| panic!("exact staged successor refused"));
        let prepared = call.prepare_replacement(&successor, basis(3000)).unwrap();
        assert!(capsule
            .replace_encoded(
                &mut call,
                &mut successor,
                capsule.node(),
                HostCallId(0),
                RequestId(1),
                b"foreign Face",
                &prepared,
                &mut output,
                encode,
                || false
            )
            .is_err());
        assert_eq!(pins.edges.get(), first_edges);
        assert!(capsule
            .replace_encoded(
                &mut call,
                &mut successor,
                capsule.node(),
                HostCallId(0),
                RequestId(1),
                &face_wire,
                &prepared,
                &mut [0; 1],
                encode,
                || false
            )
            .is_err());
        assert_eq!(pins.edges.get(), first_edges);
        assert_eq!(call.inspection().completed_operations, 1);
        pins.nack.set(nack);
        let result = capsule.replace_encoded(
            &mut call,
            &mut successor,
            capsule.node(),
            HostCallId(0),
            RequestId(1),
            &face_wire,
            &prepared,
            &mut output,
            encode,
            || false,
        );
        if nack {
            assert!(result.is_err());
            assert_eq!(call.state(), RendererState::Interrupted);
            assert_eq!(call.inspection().completed_operations, 1);
            assert_eq!(call.inspection().lifecycle, c::CapabilityLifecycle::Issued);
            pins.nack.set(false);
            call.withdraw(|| false).unwrap();
        } else {
            let (_, receipt) = result.unwrap();
            assert_eq!(
                pins.edges.get(),
                first_edges * 2,
                "replacement adds one frame, no withdrawal transaction"
            );
            assert_eq!(receipt.active(), &first);
            assert_eq!(receipt.inspection().completed_operations, 1);
            assert_eq!(
                receipt.inspection().lifecycle,
                c::CapabilityLifecycle::Revoked
            );
            let n = receipt
                .terminal_encoded::<()>(
                    &mut predecessor,
                    "fixture/replaced".into(),
                    &mut output,
                    encode,
                )
                .unwrap();
            let wire = std::str::from_utf8(&output[..n]).unwrap();
            assert!(wire.contains("replaced"));
            assert!(!wire.contains("closed"));
            assert!(call
                .closed_presented_encoded(
                    &mut predecessor,
                    "fixture/forged-closed".into(),
                    &mut output,
                    encode
                )
                .is_err());
            assert!(receipt
                .terminal_encoded::<()>(
                    &mut predecessor,
                    "fixture/repeated-replaced".into(),
                    &mut output,
                    encode
                )
                .is_err());
            assert_eq!(
                call.inspection().scope.active_play_id,
                second.active_play_id
            );
            assert_eq!(call.inspection().completed_operations, 1);
            call.withdraw(|| false).unwrap();
        }
        assert_eq!(call.inspection().completed_operations, 2);
        assert_eq!(call.inspection().scope.maximum_operations, 2);
        let mut current = Some(call);
        assert!(capsule.retire_invocation(&mut current).is_ok());
    }
}
