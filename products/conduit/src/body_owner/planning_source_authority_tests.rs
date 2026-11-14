//! Original Root-owned authority reaches owner planning; no installed action proof.
use super::*;
use conduit_core::*;
use conduit_planner::*;
use conduit_plot::*;
use conduit_std_host::{source_check::*, *};

#[path = "../../../../make/body/tests/source_check_call/support/runtime.rs"]
mod support;

#[test]
fn source_planning_uses_original_owned_grant_and_retirement_removes_it() {
    let mut projection = SourceCheckProjection::new(
        "fixture/independently-owned-source",
        1,
        include_str!("../../../../plots/hello/main.conduit"),
        conduit::source_check_worker::standard_context().unwrap(),
    )
    .unwrap();
    let request = SourceCheckRequest {
        snapshot: projection.snapshot().clone(),
        context: projection.context().clone(),
        entry: "hello".into(),
        limits: SourceCheckLimits::default(),
    };
    let bounds = SourceCheckPreparationBounds {
        request_bytes: 262_144,
        report_bytes: 65_536,
        retained_bytes: 2 * 1024 * 1024,
    };
    let prepared = PreparedSourceChecker::prepare(
        &mut [SourceCheckPreparation {
            owner: &mut projection,
            request: &request,
        }],
        bounds,
    )
    .unwrap();
    let mut host = StdHost::new();
    assert!(host.native_source_authority_grant().is_none());
    let mut definition = support::definition(host.advertisement(), &prepared);
    let grant = definition.grant.clone();
    definition.grant.boot_id = "fixture/foreign-boot".into();
    // SAFETY: the fixture owns the immutable analysis and finite immediate CPU
    // provider. Deliberate grant drift must refuse before either is installed.
    let (mut definition, provider, _) = unsafe {
        host.install_immediate_native_source(definition, Box::new(prepared.into_provider()))
    }
    .unwrap_err();
    assert!(host.native_source_authority_grant().is_none());
    definition.grant = grant.clone();
    // SAFETY: exact original Host/Boot authority and independently owned Source
    // now correspond. This is a reference preparation fixture, not IMAGE funding.
    unsafe { host.install_immediate_native_source(definition, provider) }
        .unwrap_or_else(|(_, _, error)| panic!("{error}"));
    assert_eq!(host.native_source_authority_grant(), Some(&grant));

    let source = crate::plot_source::parse(include_str!(
        "../../../../make/body/tests/source_check_call/check.conduit"
    ))
    .unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    assert!(support::plan(&host, &plot, &[], bounds)
        .unwrap_err()
        .contains("AuthorityGrantMissing"));
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let boundaries = BTreeMap::from([(
        resident.clone(),
        [
            (PortDirection::Input, "request", bounds.request_bytes),
            (PortDirection::Output, "reply", bounds.report_bytes),
        ]
        .into_iter()
        .map(|(direction, name, byte_capacity)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect(),
    )]);
    let other = crate::plot_source::parse(
        r#"plot unrelated {
 show: face/text("Message")
 "Unrelated." >> show
}."#,
    )
    .unwrap();
    let other_plot = other.expand_entry_for_authoring().unwrap();
    let other_resident = ResidentPlot::new(
        other_plot.expanded.source_document_id.clone(),
        other_plot.expanded.checked_plot_id.clone(),
    );
    let ad = host.advertisement().clone();
    let mut owner = Owner::open(host, resident, None, "Source authority fixture").unwrap();
    // Establish an already-retained workset for this reference planning fixture;
    // this is not an authorized Source-adoption demonstration.
    owner
        .session
        .admit_plot(0, other_resident, &ad.host_id, &ad.boot_id)
        .unwrap();
    let sources = [(&source, &plot), (&other, &other_plot)];
    owner
        .plan_workset_with_source_boundaries(&sources, &boundaries)
        .unwrap();
    let bindings: Vec<_> = owner
        .session
        .realization()
        .unwrap()
        .plan
        .plots
        .iter()
        .flat_map(|plot| &plot.plan.fragments)
        .flat_map(|fragment| &fragment.placements)
        .flat_map(|placement| &placement.authority)
        .collect();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].grant_id, grant.grant_id);
    assert_eq!(bindings[0].boot_id, grant.boot_id);
    assert_eq!(bindings[0].capability_id, grant.capability_id);
    assert_eq!(bindings[0].subject_kind, grant.subject_kind);
    assert_eq!(owner.session.realization().unwrap().plan.plots.len(), 2);
    assert!(owner.current_play_id().is_none());
    owner.lull().unwrap();
    // SAFETY: no Play began and the finite immediate provider is quiescent.
    unsafe { owner.host.retire_immediate_native_source() }.unwrap();
    assert!(owner.host.native_source_authority_grant().is_none());
    assert!(owner
        .plan_workset_with_source_boundaries(&sources, &boundaries)
        .is_err());
    assert!(owner.session.realization().is_none());
}
