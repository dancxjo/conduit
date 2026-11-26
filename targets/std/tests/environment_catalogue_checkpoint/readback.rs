//! Recovered seed reference drives an independently authorized checked JSON read.
use super::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    json_snapshot_file::SelectedJsonSnapshotResidence, ExternalForeDelivery, ExternalForeInput,
    ExternalForeOutputAdapter, RunControl, StdHost, StdHostConfig, TimerAdapter,
};
use std::{collections::BTreeMap, fs::File, time::Duration};
struct Output(Option<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if self.0.is_some() {
            return Err("readback exceeded its one Value output".into());
        }
        self.0 = Some(value);
        Ok(())
    }
}
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: Duration) {}
}

pub fn verify(root: &Path, recovered: &[u8]) {
    let reference = c::BoundedResourceRef::decode(recovered).unwrap();
    assert_eq!(reference.encode().unwrap(), recovered);
    let boot: c::BootId = "boot/catalogue-file-readback".into();
    let config = StdHostConfig {
        host_id: "host/catalogue-runtime".into(),
        boot_id: boot.clone(),
        offer_generation: c::OfferGeneration(1),
    };
    // An independent original read grant under this fresh current Boot, not a
    // grant reconstructed from a planned binding or producer JSON metadata.
    let grant = c::AuthorityGrant {
        grant_id: "fixture/catalogue-independent-read".into(),
        contract_id: conduit_std_offers::JSON_SNAPSHOT_AUTHORITY.into(),
        host_call_contract_id: conduit_std_offers::JSON_SNAPSHOT_READ_CALL.into(),
        subject_kind: conduit_semantic_catalog::SNAPSHOT_READ_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: boot,
        capability_id: conduit_std_offers::JSON_SNAPSHOT_READ_IMPLEMENTATION.into(),
    };
    let local = c::ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "fixture/catalogue-independent-read".into(),
        authority_contract: grant.contract_id.clone(),
        authority_grant: grant.grant_id.clone(),
        maximum_bytes: 4096,
        maximum_items: Some(1),
        availability: c::ResourceReferenceAvailability::Available,
    };
    let selected = SelectedJsonSnapshotResidence::prepare(
        File::open(root.join("snapshots")).unwrap(),
        reference,
        grant.clone(),
        local,
    )
    .unwrap();
    let mut host = StdHost::new_with_config(config);
    selected.install_offer(&mut host).unwrap();
    let hex = recovered
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let text = format!("plot catalogue-readback (\n >> reference: ResourceRef\n catalogue: JsonText >>\n) {{\n read: resource/read-json-snapshot(\"{hex}\")\n reference >> read.source\n read.value >> catalogue\n}}\n");
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    startup
        .insert_value_kind_alias("JsonText", c::kind_id(conduit_web::JSON_TEXT_INFO_ID))
        .unwrap();
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&text), &startup).unwrap();
    let source =
        expand_canonical_plot_for_authoring(&checked, "catalogue-readback", &profiles).unwrap();
    assert_eq!(source.expanded.gears.len(), 1);
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&source.expanded, &hosts).unwrap();
    let boundaries = [
        (
            c::PortDirection::Input,
            "reference",
            c::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
        ),
        (c::PortDirection::Output, "catalogue", 4096),
    ]
    .into_iter()
    .map(|(direction, name, byte_capacity)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: c::port_id(name),
                track: c::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity,
            },
        )
    })
    .collect::<BTreeMap<_, _>>();
    let empty = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &source,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: core::slice::from_ref(&grant),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert!(plan.fragments[0].states.is_empty());
    let input = ExternalForeInput {
        front_port_id: "reference".into(),
        track: c::ConnectionTrack::Payload,
        bytes: recovered.to_vec(),
    };
    let mut output = Output(None);
    let report = host
        .run_body_durable_json_snapshot_to(
            &execution::body(BODY),
            &root.join("state"),
            &selected,
            plan.fragments[0].clone(),
            &[input],
            1,
            &mut output,
            &mut Vec::new(),
            &mut Timer,
            &RunControl::default(),
        )
        .unwrap();
    assert_eq!(
        report.kernel.as_ref().unwrap().post_play_start_allocations,
        0
    );
    let result = output.0.expect("actual JSON read result");
    assert_eq!(result.bytes.as_ref(), catalogue_bytes());
    let mut reader = catalogue::PreparedRecordingCatalogReader::default();
    reader
        .read(std::str::from_utf8(result.bytes.as_ref()).unwrap())
        .unwrap();
    assert_eq!(reader.len(), 16);
    for (index, seed) in (1..=16).enumerate() {
        assert_eq!(
            reader.encoded_reference(index),
            Some(chunk_reference(seed).encode().unwrap().as_slice())
        );
    }
}
