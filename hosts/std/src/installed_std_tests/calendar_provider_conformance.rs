use super::{installed_std, RecordingTimer};
use crate::hosted_calendar::{CalendarHostedOperation, HostedCalendarAdapter};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    kind_id, BaseImplementationId, BootId, ConfigurationValue, HostId, OfferGeneration,
    PortDirection, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

struct RecordingCalendar {
    expected: Vec<CalendarHostedOperation>,
    results: Vec<Vec<u8>>,
}

impl HostedCalendarAdapter for RecordingCalendar {
    fn execute(
        &mut self,
        operation: CalendarHostedOperation,
        semantic_json: &[u8],
        prior_realization_json: Option<&[u8]>,
    ) -> Result<Vec<u8>, crate::hosted_calendar::GoogleCalendarRefusal> {
        assert_eq!(Some(operation), self.expected.first().copied());
        self.expected.remove(0);
        assert!(serde_json::from_slice::<serde_json::Value>(semantic_json).is_ok());
        if matches!(
            operation,
            CalendarHostedOperation::Update
                | CalendarHostedOperation::Cancel
                | CalendarHostedOperation::Invite
        ) {
            assert!(prior_realization_json.is_some());
        } else {
            assert!(prior_realization_json.is_none());
        }
        Ok(self.results.remove(0))
    }
}

#[test]
fn read_and_free_busy_use_distinct_authority_through_plan_and_play() {
    for (operation, kind, output_port, result_type, result) in [
        (
            CalendarHostedOperation::Read,
            conduit_std_catalog::CALENDAR_READ_KIND,
            "events",
            conduit_std_catalog::calendar_read_result_type(),
            br#"{"resource":{"account_identity":"account/alice","calendar_id":"primary"},"events":[],"next_page_token":null}"#.to_vec(),
        ),
        (
            CalendarHostedOperation::FreeBusy,
            conduit_std_catalog::CALENDAR_FREE_BUSY_KIND,
            "availability",
            conduit_std_catalog::calendar_free_busy_result_type(),
            br#"{"resource":{"account_identity":"account/alice","calendar_id":"primary"},"observed_unix_seconds":7,"usable_until_unix_seconds":67,"participants":[]}"#.to_vec(),
        ),
    ] {
        let source = format!(
            "form proof {{\n provider: {kind}(request = {{semantic_json: \"{{}}\"}})\n sink: conduit-test/structured-sink(value = \"{}\")\n provider.{output_port} > sink.input\n}}\n",
            hex(&result_value(result_type.clone(), &result))
        );
        let (startup, profile, sink) = catalogs(&result_type);
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "proof", &profile).unwrap();
        let mut host = calendar_host(RecordingCalendar {
            expected: vec![operation],
            results: vec![result],
        });
        let mut advertisement = host.advertisement().clone();
        advertisement.capabilities.push(sink);
        advertisement.capabilities.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let hosts = [advertisement.clone()];
        let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
        let grants = host.calendar_authority_grants(operation, "calendar-proof").unwrap();
        let plan = conduit_planner::plan_expanded_canonical_with_options(
            &expanded,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES,
                authority_grants: &grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
        run(&mut host, advertisement, plan.fragments[0].clone());
    }
}

#[test]
fn authorized_create_update_and_cancel_chain_exact_receipts_through_plan_and_play() {
    let write = br#"{"resource":{"account_identity":"account/alice","calendar_id":"primary"},"portable_event_identity":"event/review","provider_event_id":"google-7","provider_revision":"etag-7","event":{"summary":"Review"}}"#.to_vec();
    let updated = br#"{"resource":{"account_identity":"account/alice","calendar_id":"primary"},"portable_event_identity":"event/review","provider_event_id":"google-7","provider_revision":"etag-8","event":{"summary":"Review moved"}}"#.to_vec();
    let cancelled = br#"{"resource":{"account_identity":"account/alice","calendar_id":"primary"},"portable_event_identity":"event/review","provider_event_id":"google-7","cancelled_revision":"etag-8"}"#.to_vec();
    let expected = result_value(
        conduit_std_catalog::calendar_cancel_receipt_type(),
        &cancelled,
    );
    let source = format!(
        "form proof {{\n create: calendar/create-event(request = {{semantic_json: \"{{}}\"}})\n update: calendar/update-event(request = {{semantic_json: \"{{}}\"}})\n cancel: calendar/cancel-event(request = {{semantic_json: \"{{}}\"}})\n sink: conduit-test/structured-sink(value = \"{}\")\n create.receipt > update.prior\n update.receipt > cancel.prior\n cancel.receipt > sink.input\n}}\n",
        hex(&expected)
    );
    let result_type = conduit_std_catalog::calendar_cancel_receipt_type();
    let (startup, profile, sink) = catalogs(&result_type);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "proof", &profile).unwrap();
    let mut host = calendar_host(RecordingCalendar {
        expected: vec![
            CalendarHostedOperation::Create,
            CalendarHostedOperation::Update,
            CalendarHostedOperation::Cancel,
        ],
        results: vec![write, updated, cancelled],
    });
    let mut advertisement = host.advertisement().clone();
    advertisement.capabilities.push(sink);
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let mut grants = Vec::new();
    for operation in [
        CalendarHostedOperation::Create,
        CalendarHostedOperation::Update,
        CalendarHostedOperation::Cancel,
    ] {
        grants.extend(
            host.calendar_authority_grants(operation, operation.contract())
                .unwrap(),
        );
    }
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    run(&mut host, advertisement, plan.fragments[0].clone());
}

fn calendar_host(adapter: RecordingCalendar) -> StdHost {
    StdHost::new_with_calendar(
        StdHostConfig {
            host_id: HostId::from("calendar-host"),
            boot_id: BootId::from("calendar-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(adapter),
    )
    .unwrap()
}

fn run(
    host: &mut StdHost,
    advertisement: conduit_core::HostAdvertisement,
    fragment: conduit_core::PlanFragment,
) {
    let mut output = Vec::with_capacity(4_096);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let mut signs = 0;
    let report = installed_std::run_fragment(
        installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: host.calendar.as_deref_mut(),
        },
        &fragment,
        0,
        &mut signs,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .unwrap();
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

fn catalogs(
    value_type: &StructuredInfoType,
) -> (
    StartupCatalog,
    ProfileCatalog,
    conduit_core::CapabilityOffer,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_calendar_provider_catalogs(&mut startup, &mut profile).unwrap();
    let mut sink = installed_std::test_structured_selector::offer(value_type, PortDirection::Input);
    sink.inputs[0].temporal = conduit_core::PortTemporal::Value;
    startup
        .insert(KindSignature {
            kind: installed_std::test_structured_selector::SINK_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: Some("\"\"".into()),
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: vec![],
            configuration: vec![ConfigurationField {
                key: "value".into(),
                default_value: ConfigurationValue::Text(String::new()),
                validation: ConfigurationRule::TextBytes {
                    maximum: conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES * 2,
                },
            }],
        })
        .unwrap();
    (startup, profile, sink)
}

fn result_value(value_type: StructuredInfoType, json: &[u8]) -> Vec<u8> {
    let leaf = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1")).unwrap(),
        json.to_vec(),
    )
    .unwrap();
    StructuredInfoValue::record(
        value_type,
        vec![StructuredFieldValue::new("realization_json", leaf).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
