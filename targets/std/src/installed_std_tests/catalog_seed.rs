//! The appliance's authored seed through the existing installed durable State.
//! Host offers and Body admission are CPU fixtures; actual files survive a fresh
//! process. This does not publish a catalogue or confer snapshot authority.
use super::{host, BTreeMap, RecordingTimer};
use crate::{ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, ExternalForeRun};
use conduit_core as c;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};

#[derive(Default)]
pub(super) struct Collector {
    pub(super) values: Vec<ExternalForeDelivery>,
    pub(super) stop_after: Option<Vec<u8>>,
    pub(super) control: crate::RunControl,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        let stop = self
            .stop_after
            .as_ref()
            .is_some_and(|expected| value.bytes.as_slice() == expected);
        self.values.push(value);
        if stop {
            self.stop_after = None;
            self.control
                .request_stop(crate::RunControlRequestId::new("catalog-current-observed").unwrap())
                .map_err(|error| format!("stop inspected seed: {error:?}"))?;
        }
        Ok(())
    }
}
pub(super) fn plan(write: bool, boot: &str) -> (crate::StdHost, c::Plan, c::StructuredInfoType) {
    let source = if write {
        include_str!("../../../../bodies/environment/profiles/catalog-seed.conduit")
    } else {
        include_str!("../../../../bodies/environment/profiles/catalog-seed-read.conduit")
    };
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let authoring = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "recorder-catalog",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let (mut host, value_type) = configured_host(&authoring, boot);
    let plan = plan_on_host(&authoring, &mut host, &[], 1024);
    (host, plan, value_type)
}
pub(super) fn configured_host(
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    boot: &str,
) -> (crate::StdHost, c::StructuredInfoType) {
    let seed = authoring
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == c::STATE_VALUE_KIND)
        .unwrap();
    let c::ConfigurationValue::Structured(initial) = &seed
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .unwrap()
        .value
    else {
        panic!("canonical seed")
    };
    let initial = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    let value_type = initial.value_type().clone();
    let mut host = host("fixture/recorder-seed-host");
    host.advertisement.boot_id = boot.into();
    host.advertisement
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != c::STATE_VALUE_KIND);
    host.advertisement.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer(
            seed.startup_parameters[0].value_type.as_str(),
            &value_type,
            &initial,
        )
        .unwrap(),
    );
    host.advertisement.resources.push(c::ResourceOffer {
        pool_id: "fixture/durable-catalog-state".into(),
        class_id: conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS.into(),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    (host, value_type)
}
pub(super) fn plan_on_host(
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    host: &mut crate::StdHost,
    grants: &[c::AuthorityGrant],
    bytes: u32,
) -> c::Plan {
    host.advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host.advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement).unwrap();
    let hosts = [host.advertisement.clone()];
    let choices =
        conduit_planner::default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &authoring.input_bindings),
        (c::PortDirection::Output, &authoring.output_bindings),
    ] {
        for binding in bindings {
            boundaries.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: bytes,
            authority_grants: grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments[0].states[0].lifetime, c::StateLifetime::Body);
    plan
}
fn reference() -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([11; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 1024,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([12; 32]),
            expires_at: None,
        },
    }
}
#[test]
fn catalog_seed_survives_a_fresh_process_without_replacing_the_value() {
    const PHASE: &str = "CONDUIT_CATALOG_SEED_PROOF_PHASE";
    const ROOT: &str = "CONDUIT_CATALOG_SEED_PROOF_ROOT";
    let Ok(phase) = std::env::var(PHASE) else {
        let root = tempfile::tempdir().unwrap();
        let mut published: Option<(std::path::PathBuf, Vec<u8>)> = None;
        for phase in ["write", "read", "foreign-body", "corrupt"] {
            if phase == "corrupt" {
                let (path, bytes) = published.as_ref().unwrap();
                std::fs::write(path, &bytes[..bytes.len() - 1]).unwrap();
            }
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "installed_std_tests::catalog_seed::catalog_seed_survives_a_fresh_process_without_replacing_the_value", "--nocapture"])
                .env(PHASE, phase).env(ROOT, root.path()).status().unwrap();
            assert!(status.success(), "catalog seed phase {phase}");
            if phase == "write" {
                let files: Vec<_> = std::fs::read_dir(root.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().path().join("current"))
                    .filter(|path| path.is_file())
                    .collect();
                assert_eq!(files.len(), 1);
                published = Some((files[0].clone(), std::fs::read(&files[0]).unwrap()));
            } else {
                let (path, bytes) = published.as_ref().unwrap();
                let expected = if phase == "corrupt" {
                    &bytes[..bytes.len() - 1]
                } else {
                    bytes.as_slice()
                };
                assert_eq!(
                    std::fs::read(path).unwrap(),
                    expected,
                    "read-only recovery must not replace the committed generation"
                );
            }
        }
        return;
    };
    let write = phase == "write";
    let (mut host, plan, value_type) = plan(write, &format!("fixture/seed-boot/{phase}"));
    let reference = reference();
    let payload = c::StructuredInfoValue::leaf(
        c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap(),
        reference.encode().unwrap(),
    )
    .unwrap();
    let selected = c::StructuredInfoValue::variant(value_type.clone(), "some", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let inputs = if write {
        vec![ExternalForeInput {
            front_port_id: "committed".into(),
            track: c::ConnectionTrack::Payload,
            bytes: selected.clone(),
        }]
    } else {
        vec![]
    };
    let body: conduit_body::BodyId = serde_json::from_str(if phase == "foreign-body" {
        "\"fixture/other-body\""
    } else {
        "\"fixture/recorder-body\""
    })
    .unwrap();
    let expected = if phase == "foreign-body" {
        c::StructuredInfoValue::variant(
            value_type,
            "none",
            c::StructuredInfoValue::leaf(
                c::StructuredInfoType::leaf(c::kind_id(c::EMPTY_INFO_ID)).unwrap(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    } else {
        selected.clone()
    };
    let control = crate::RunControl::default();
    let mut collector = Collector {
        stop_after: Some(expected),
        control: control.clone(),
        ..Collector::default()
    };
    let result = host.run_body_durable_external_plot_to(
        &body,
        &std::path::PathBuf::from(std::env::var_os(ROOT).unwrap()),
        plan.fragments[0].clone(),
        ExternalForeRun {
            inputs: &inputs,
            output: &mut collector,
            sequential: false,
            stream: None,
        },
        &mut Vec::new(),
        &mut RecordingTimer { waits: Vec::new() },
        &control,
    );
    if phase == "corrupt" {
        assert!(
            result.is_err(),
            "corrupt seed must refuse, never reset to empty"
        );
        assert!(collector.values.is_empty());
        return;
    }
    let report = result.unwrap();
    assert!(report.kernel.is_some());
    assert_eq!(report.control_receipts.len(), 1);
    let actual = collector.values.last().expect("current seed delivery");
    if phase == "foreign-body" {
        let value = c::StructuredInfoValue::from_canonical_bytes(&actual.bytes).unwrap();
        assert!(matches!(
            value.shape(),
            c::StructuredInfoValueShape::Variant { tag: "none", .. }
        ));
    } else {
        assert_eq!(actual.bytes.as_slice(), selected);
        if phase == "read" {
            assert!(collector
                .values
                .iter()
                .all(|delivery| delivery.bytes.as_slice() == selected));
        }
    }
}
