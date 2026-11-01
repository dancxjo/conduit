//! Actual selected Body State execution, not installed recorder or history ACK proof.
use conduit_core::*;
use conduit_std_host::{
    body_execution::*, ExternalForeInput, RunControl, StdHost, StdHostConfig, ThreadTimer,
};
use std::collections::BTreeMap;
#[path = "common/checkpoint_fixture.rs"]
mod checkpoint_fixture;
use checkpoint_fixture::{body_plan, CapturedFore};
struct BorrowedCapture(Vec<Vec<u8>>);
impl BodyBorrowedForeOutputAdapter for BorrowedCapture {
    fn deliver(&mut self, output: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.0.len() == 2 {
            return Err("fixture's independently finite custody is full".into());
        }
        self.0.push(output.bytes.to_vec());
        Ok(())
    }
}
fn planned(boot: &str) -> (StdHost, Plan, ResourceOffer) {
    let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(ty.clone(), Vec::new()).unwrap();
    let mut advertisement = StdHost::new_with_config(StdHostConfig {
        host_id: "state/body-host".into(),
        boot_id: boot.into(),
        offer_generation: OfferGeneration(1),
    })
    .advertisement()
    .clone();
    advertisement.capabilities.retain(|offer| {
        offer.kind_id.as_str() != conduit_semantic_catalog::state_value::STATE_VALUE_KIND
    });
    advertisement
        .capabilities
        .push(conduit_std_offers::state_value_durable_std_offer("Text", &ty, &initial).unwrap());
    let resource = ResourceOffer {
        pool_id: "state/body-pool".into(),
        class_id: conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS.into(),
        capacity_units: 1,
        compute: None,
        content: None,
    };
    advertisement.resources.push(resource.clone());
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let (host, plan) = planned_on(host);
    (host, plan, resource)
}
fn checked_source() -> (
    conduit_plot::CheckedSyntaxDocument,
    conduit_plot::ExpandedAuthoringPlot,
    conduit_plot::ProfileCatalog,
) {
    let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(ty.clone(), Vec::new()).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    startup
        .insert_value_kind_alias("Text", kind_id(TEXT_INFO_ID))
        .unwrap();
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "Text",
        &ty,
        &initial,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked = conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document("plot durable (\n >> write: Text\n current: $Text >>\n) {\n cell: keep Text(\"\") <= 4KiB for life\n write >> cell\n cell >> current\n}\n"), &startup).unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "durable", &profiles).unwrap();
    (checked, authored, profiles)
}
fn planned_on(host: StdHost) -> (StdHost, Plan) {
    let (checked, authored, profiles) = checked_source();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = BTreeMap::from([
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("write"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("current"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        ),
    ]);
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &authored,
        &profiles,
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 2,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (host, plan)
}
fn thread(test: fn()) {
    std::thread::Builder::new()
        .stack_size(4 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn actual_body_state_recovers_after_fresh_host_boot() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        let mut original_body = None;
        for (boot, input) in [
            ("state/boot-a", Some(b"remember this".as_slice())),
            ("state/boot-b", Some(b"after recovery".as_slice())),
        ] {
            let (mut host, plan, resource) = planned(boot);
            let (wake, body) = body_plan(plan);
            if let Some(id) = original_body.as_ref() {
                assert_eq!(id, &body.body_id);
            } else {
                original_body = Some(body.body_id.clone());
            }
            let inputs: Vec<_> = input
                .into_iter()
                .map(|bytes| ExternalForeInput {
                    front_port_id: port_id("write"),
                    track: ConnectionTrack::Payload,
                    bytes: bytes.to_vec(),
                })
                .collect();
            let mut fore = BorrowedCapture(Vec::with_capacity(2));
            let report = host
                .run_body_plan_with_durable_state_borrowed_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &body,
                        control: &RunControl::default(),
                        keyboard: None,
                    },
                    &inputs,
                    &mut fore,
                    BodyDurableStateSelection {
                        root: root.path(),
                        resource: &resource,
                        authorities: &[],
                        body_id: &body.body_id,
                        plan_id: &body.plan_id,
                    },
                    #[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
                    None,
                    &mut Vec::new(),
                    &mut ThreadTimer,
                    |play, _| {
                        assert!(play.validate_for(&body));
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(
                report.terminal,
                TerminalDisposition::Completed,
                "{:?}",
                report.failure
            );
            assert!(report.cleanup_failure.is_none());
            if boot == "state/boot-b" {
                assert_eq!(fore.0.first().unwrap().as_slice(), b"remember this");
            }
            assert!(
                fore.0.iter().any(|row| row.as_slice() == b"remember this"),
                "{:?}",
                fore.0
            );
        }
    });
}
#[test]
fn rejected_start_has_no_state_effect_and_releases_original_pool() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        let (mut host, plan, resource) = planned("state/boot-a");
        let (wake, body) = body_plan(plan);
        let mut default_fore = CapturedFore::default();
        let default_result = host.run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[],
            false,
            &mut default_fore,
            &mut Vec::new(),
            &mut ThreadTimer,
        );
        assert!(
            default_result.is_err(),
            "unselected Body State must keep refusing"
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        for accept in [false, true] {
            let mut fore = BorrowedCapture(Vec::with_capacity(2));
            let result = host.run_body_plan_with_durable_state_borrowed_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &body,
                    control: &RunControl::default(),
                    keyboard: None,
                },
                &[ExternalForeInput {
                    front_port_id: port_id("write"),
                    track: ConnectionTrack::Payload,
                    bytes: b"accepted".to_vec(),
                }],
                &mut fore,
                BodyDurableStateSelection {
                    root: root.path(),
                    resource: &resource,
                    authorities: &[],
                    body_id: &body.body_id,
                    plan_id: &body.plan_id,
                },
                #[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
                None,
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
                    if accept {
                        Ok(())
                    } else {
                        Err("fixture rejected start".into())
                    }
                },
            );
            if accept {
                assert_eq!(result.unwrap().terminal, TerminalDisposition::Completed);
            } else {
                let error = result.unwrap_err();
                assert!(error.contains("fixture rejected start"), "{error}");
                assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
            }
        }
    });
}

#[test]
fn foreign_selected_resource_refuses_before_start_or_residence_effect() {
    thread(|| {
        let root = tempfile::tempdir().unwrap();
        let (mut host, plan, mut resource) = planned("state/boot-a");
        let (wake, body) = body_plan(plan);
        resource.pool_id = "state/foreign-pool".into();
        let mut fore = CapturedFore::default();
        let mut started = false;
        let result = host.run_body_plan_with_durable_state_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &body,
                control: &RunControl::default(),
                keyboard: None,
            },
            BodyForeExchange {
                inputs: &[],
                output: &mut fore,
            },
            BodyDurableStateSelection {
                root: root.path(),
                resource: &resource,
                authorities: &[],
                body_id: &body.body_id,
                plan_id: &body.plan_id,
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                started = true;
                Ok(())
            },
        );
        assert!(result.unwrap_err().contains("Body State selection differs"));
        assert!(!started);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    });
}

#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
#[path = "body_durable_state_play/selected_storage.rs"]
mod selected_storage;
