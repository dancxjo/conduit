//! Exact resource-reference correlation through the installed ordinary kernel.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::*;
use std::collections::BTreeMap;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("no timer effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
#[test]
fn checked_reference_equality_executes_through_installed_kernel() {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    let source = "type Pair = {\n    left: ResourceRef\n    right: ResourceRef\n}\nplot equal (\n    >> input: Pair\n    result: Boolean >>\n) = (.left == .right)";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "equal", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let offer =
        conduit_std_offers::pure_expression_std_offer(&program, PortTemporal::Value).unwrap();
    let base = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "resource-equality-host".into(),
            boot_id: "resource-equality-boot".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let mut advertisement = base.advertisement().clone();
    advertisement.capabilities.push(offer);
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "input"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "lexicon/test@1".into(),
        access_class: "resource/read@1".into(),
        extent: ResourceExtent {
            bytes: 12,
            items: Some(2),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    for same_version in [true, false] {
        let mut other = reference.clone();
        if !same_version {
            other.lifetime.version = ResourceVersionIdentity::from_digest([3; 32]);
        }
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "left",
                    StructuredInfoValue::leaf(
                        field(&program.input_type, "left").clone(),
                        reference.encode().unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "right",
                    StructuredInfoValue::leaf(
                        field(&program.input_type, "right").clone(),
                        other.encode().unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let mut outputs = Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "input".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input,
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0.len(), 1);
        assert_eq!(
            InfoBool::decode(&outputs.0[0].bytes).unwrap().get(),
            same_version
        );
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::BackCompleted));
        // The selected Std expression Back crosses its admitted generic Host
        // Call. No resource-read grant or resource residence is installed.
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
        assert!(kernel
            .kernel_sign
            .iter()
            .all(|event| !matches!(event.kind, conduit_kernel::KernelEventKind::BackFailed)));
    }
}
