use super::*;
use conduit_core::{seal_plan, PlotIdentity};

#[test]
fn empty_composite_is_refused_before_any_child_is_admitted() {
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: "source".into(),
            checked_plot_id: "checked".into(),
            expanded_plot_id: "expanded".into(),
        },
        vec![],
    );
    assert_eq!(
        KernelCompositePreparation::prepare(plan),
        Err(KernelCompositeError::Empty)
    );
}

#[test]
fn host_call_obligation_commits_every_routing_identity() {
    let exact =
        host_call_obligation_identity("plan", "fragment", "placement", HostCallId(0), "contract");
    for drifted in [
        host_call_obligation_identity("other", "fragment", "placement", HostCallId(0), "contract"),
        host_call_obligation_identity("plan", "other", "placement", HostCallId(0), "contract"),
        host_call_obligation_identity("plan", "fragment", "other", HostCallId(0), "contract"),
        host_call_obligation_identity("plan", "fragment", "placement", HostCallId(1), "contract"),
        host_call_obligation_identity("plan", "fragment", "placement", HostCallId(0), "other"),
    ] {
        assert_ne!(drifted, exact);
    }
}

#[test]
fn dispatch_token_refuses_unknown_stale_and_replayed_tokens_without_aliasing_live_tokens() {
    use conduit_kernel::{BoundedValueRef, RequestId, ValueRef};
    let request = HostCallRequest {
        node: NodeId(1),
        request: RequestId(2),
        call: HostCallId(3),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 4,
                generation: 5,
                byte_len: 6,
            },
            6,
        )
        .unwrap(),
    };
    let identity = [7; 32];
    let mut outstanding = vec![
        Some(OutstandingHostCall {
            token: 9,
            child_index: 0,
            request,
            obligation_identity: identity,
        }),
        Some(OutstandingHostCall {
            token: 10,
            child_index: 1,
            request: HostCallRequest {
                request: RequestId(8),
                ..request
            },
            obligation_identity: [8; 32],
        }),
    ];
    let exact = KernelCompositeHostRequest { dispatch_token: 9 };
    let other_live = KernelCompositeHostRequest { dispatch_token: 10 };
    assert_eq!(
        outstanding_host_call_index(&outstanding, exact.dispatch_token),
        Ok(0)
    );
    assert_eq!(
        outstanding_host_call_index(&outstanding, other_live.dispatch_token),
        Ok(1)
    );
    assert!(outstanding_host_call_index(&outstanding, 11).is_err());
    outstanding[0] = None;
    assert!(outstanding_host_call_index(&outstanding, exact.dispatch_token).is_err());
    assert!(outstanding_host_call_index(&outstanding, exact.dispatch_token).is_err());
    assert_eq!(
        outstanding_host_call_index(&outstanding, other_live.dispatch_token),
        Ok(1)
    );
}

#[test]
fn dispatch_requires_exact_current_host_resources_and_authority() {
    use conduit_core::{
        AuthorityContractId, AuthorityGrantId, BootId, CapabilityId, HostCallContractId, KindId,
        OfferGeneration, ResourceClassId, ResourcePoolId,
    };
    let host = PreparationHostIdentity {
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(1),
    };
    let resource = ResourceBinding {
        pool_id: ResourcePoolId::from("pool"),
        class_id: ResourceClassId::from("class"),
        units: 1,
        protected: None,
        compute: None,
        content: None,
    };
    let authority = AuthorityBinding {
        grant_id: AuthorityGrantId::from("grant"),
        contract_id: AuthorityContractId::from("authority"),
        host_call_contract_id: HostCallContractId::from("call"),
        subject_kind: KindId::from("subject"),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CapabilityId::from("capability"),
    };
    let exact = KernelCompositeHostCallObligation {
        host: host.clone(),
        requirement: HostCallRequirement {
            contract_id: HostCallContractId::from("call"),
            target_kind: Some(KindId::from("subject")),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 1,
        },
        resources: vec![resource.clone()],
        authorities: vec![authority.clone()],
    };
    assert!(dispatch_matches(
        &exact,
        &host,
        core::slice::from_ref(&resource),
        core::slice::from_ref(&authority)
    ));
    assert!(!dispatch_matches(
        &exact,
        &host,
        &[],
        core::slice::from_ref(&authority)
    ));
    assert!(!dispatch_matches(
        &exact,
        &host,
        core::slice::from_ref(&resource),
        &[]
    ));
    let mut stale = host;
    stale.offer_generation = OfferGeneration(2);
    assert!(!dispatch_matches(
        &exact,
        &stale,
        core::slice::from_ref(&resource),
        core::slice::from_ref(&authority)
    ));
}

#[test]
fn fixed_dispatch_slots_reuse_without_capacity_growth() {
    let mut slots: Vec<Option<OutstandingHostCall>> = (0..2).map(|_| None).collect();
    let capacity = slots.capacity();
    for token in 0..32 {
        slots[0] = Some(OutstandingHostCall {
            token,
            child_index: 0,
            request: HostCallRequest {
                node: NodeId(0),
                request: conduit_kernel::RequestId(token as u32),
                call: HostCallId(0),
                input: conduit_kernel::BoundedValueRef::new(
                    conduit_kernel::ValueRef {
                        slot: 0,
                        generation: 0,
                        byte_len: 0,
                    },
                    0,
                )
                .unwrap(),
            },
            obligation_identity: [0; 32],
        });
        slots[0] = None;
    }
    assert_eq!(slots.capacity(), capacity);
    assert_eq!(slots.len(), 2);
}
