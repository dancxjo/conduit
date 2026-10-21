//! A fresh local copy must not accept a retired arena's lease issuance.
#[path = "resource_common/provider.rs"]
mod resource_provider;
use conduit_core::*;
use conduit_planner::proof::resource_frame::*;
use conduit_std_host::hosted_resource::ResourcePublicationRefusal;
#[test]
fn identical_content_versions_still_require_distinct_concrete_copy_handles() {
    let case = frame_resource_plan(false, false).unwrap();
    let gear = case.plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "frame/source")
        .unwrap();
    let reader: AuthorityGrantId = "copy-reader".into();
    let payload = vec![7; FRAME_BYTES as usize];
    let mut first = resource_provider::provider(&gear.resources[0], &reader, None, None)
        .initialize(&payload)
        .unwrap();
    let retired = first.acquire(&reader).unwrap();
    first.release(retired).unwrap();
    drop(first);
    let mut next = resource_provider::provider_with_handle(
        &gear.resources[0],
        &reader,
        None,
        None,
        "fresh-copy-handle".into(),
    )
    .initialize(&payload)
    .unwrap();
    let current = next.acquire(&reader).unwrap();
    assert_eq!(
        next.read(retired),
        Err(ResourcePublicationRefusal::StaleLease)
    );
    assert_eq!(next.read(current).unwrap(), payload);
    next.release(current).unwrap();
}
