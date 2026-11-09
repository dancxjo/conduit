#![cfg(feature = "execution")]
//! Real signed six-Plot reference through the public local admission/Start lane.
//! The prepared CPU ticket has no hardware or IO authority; it counts transfers.
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    continuation_native::ContinuationNativeContext,
    execution::{Policy, Refusal},
    execution_workset::{PreparedLocalKernels, PreparedLocalWorkset},
};
use std::cell::Cell;
#[path = "../../../../architecture/body/tests/continuation_scope/reference.rs"]
mod reference;
struct Ticket<'a>(&'a Cell<usize>);
// SAFETY: inert CPU ticket has no external backing or effects, and its only
// binding operation counts the affine transfer after the public Start gate.
unsafe impl PreparedLocalKernels for Ticket<'_> {
    type Bound = ();
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    unsafe fn bind(self) {
        self.0.set(self.0.get() + 1);
    }
}
#[test]
fn checked_continuation_uses_atomic_admission_distinct_origin_and_one_use_start() {
    reference::with_reference(|scope, verified, actual| {
        let intent = verified.intent();
        let body = serde_json::from_value(serde_json::json!(intent.body_id)).unwrap();
        let bootstrap = b::MemberBootstrap {
            schema: b::MEMBER_BOOTSTRAP_SCHEMA.into(),
            body_id: body,
            host_id: intent.host_id.into(),
            image_id: intent.image_id.into(),
            configuration_id: intent.configuration_id.into(),
            owner: b::FederationPeer {
                host_id: intent.issuer_host.into(),
                boot_id: "owner/retained".into(),
                offer_generation: c::OfferGeneration(1),
                verifying_key: intent.issuer_key,
            },
            admission: b::MemberBootstrapMaterial::Retained {
                credential: b::MembershipCredential {
                    body_id: serde_json::from_value(serde_json::json!(intent.body_id)).unwrap(),
                    credential_id: serde_json::from_value(serde_json::json!(intent.credential_id))
                        .unwrap(),
                    part_id: serde_json::from_value(serde_json::json!(intent.part_id)).unwrap(),
                    host_id: actual.host_id.clone(),
                    boot_id: "previous/boot".into(),
                    issued_at_millis: 1,
                },
                secret: [3; 32],
            },
        };
        let preparation = b::HostPreparationDelegation {
            authorizer: b::HostPreparationAuthorizer::SeedOwner,
            maximum_assignment_bytes: intent.ceilings.assignment_bytes,
            maximum_runtime_state_bytes: intent.ceilings.runtime_bytes,
        };
        let execution = b::HostExecutionDelegation {
            authorizer: b::HostExecutionAuthorizer::SeedOwner,
            maximum_active_worksets: 1,
            continuation: Some(intent.ceilings),
        };
        let policy = Policy {
            preparation: Some(&preparation),
            execution: Some(&execution),
            active_worksets: 0,
            maximum_retained_bytes: intent.ceilings.runtime_bytes as usize,
        };
        let context = ContinuationNativeContext::check_installation(
            &bootstrap,
            actual,
            verified,
            intent.image_id,
            intent.configuration_id,
            policy,
        )
        .unwrap();
        let binds = Cell::new(0);
        let prepare = || {
            PreparedLocalWorkset::<_, 3>::prepare_continuation_selected::<0, ()>(
                scope,
                context,
                policy,
                |_, _| {
                    Ok(b::AssignedBodyPreparationProfile {
                        maxima: c::AssignedPlanMaxima {
                            encoded_bytes: 4096,
                            runtime_state_bytes: 4096,
                            counts: [255; 13],
                        },
                        requirements: c::AssignedPlanRequirements {
                            host: c::AssignedIdentity::from_text(actual.host_id.as_str()),
                            boot: c::AssignedIdentity::from_text(actual.boot_id.as_str()),
                            host_calls: &[],
                            resources: &[0],
                            remote_bindings: &[],
                        },
                        computations: c::AssignedComputationProfile {
                            maximum_text_bytes: 0,
                            maximum_total_text_bytes: 0,
                            requirements: &[],
                        },
                        fore: None,
                    })
                },
                |fragments, _| {
                    assert_eq!(fragments.len(), 3);
                    assert!(fragments.iter().all(|f| scope.matches_fragment(f)));
                    Ok(Ticket(&binds))
                },
            )
            .unwrap()
        };
        let mut resources = c::ResourceAdmissionOwner::new(actual.clone());
        let before = resources.clone();
        assert!(prepare().admit::<()>(&mut resources, &[], 0).is_err());
        assert_eq!(resources, before, "refused admission is atomic");
        assert_eq!(binds.get(), 0);
        let pool = &actual.resources[0];
        let observed = c::ResourceObservation {
            host_id: actual.host_id.clone(),
            boot_id: actual.boot_id.clone(),
            offer_generation: actual.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: c::ResourceHealth::Ready,
            unreserved_units: 3,
            utilized_units: 0,
            sign_id: "observed/current".into(),
        };
        let mut admitted = prepare()
            .admit::<()>(&mut resources, &[observed], 0)
            .unwrap();
        assert_eq!(resources.admissions().len(), 3);
        let missing_ledger = c::ResourceAdmissionOwner::new(actual.clone());
        assert!(matches!(
            admitted.start_continuation::<()>(scope, context, policy, &missing_ledger),
            Err(Refusal::Scope)
        ));
        assert_eq!(binds.get(), 0, "refused Start retains original ticket");
        let workspace = b::HostCommandInput::<1>::new();
        // Deliberately old retained identity and EMPTY online input. A continuation
        // must reject its origin before attempting to reinterpret these as a stamp.
        let online = b::NativePreparationContext {
            bootstrap: &bootstrap,
            member: bootstrap.admission.credential().unwrap(),
            actual,
            image_id: intent.image_id,
            configuration_id: intent.configuration_id,
            now_millis: 1,
        };
        assert!(matches!(
            admitted.start::<1, ()>(&workspace, online, policy, &resources),
            Err(Refusal::Scope)
        ));
        assert_eq!(binds.get(), 0);
        let started = admitted
            .start_continuation::<()>(scope, context, policy, &resources)
            .unwrap();
        assert_eq!(*started.custody_identity(), scope.custody_identity());
        assert_eq!(binds.get(), 1);
        assert!(matches!(
            admitted.start_continuation::<()>(scope, context, policy, &resources),
            Err(Refusal::Scope)
        ));
        assert_eq!(binds.get(), 1, "Start cannot consume the same ticket twice");
    });
}
