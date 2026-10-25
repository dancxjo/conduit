//! Lifecycle gate and retained Host custody, not native execution proof.
use super::*;
use conduit_body::ResidentPlot;
use conduit_std_host::StdHost;

const SOURCE: &str = "plot hello {\n show: face/text(\"Message\")\n \"Hello.\" >> show\n}.";
fn owner() -> (Owner, conduit_plot::ExpandedAuthoringPlot) {
    let checked = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    let owner = Owner::open(StdHost::new(), resident, None, "Retained Body").unwrap();
    (owner, checked)
}
fn slot() -> ResourceOffer {
    conduit_core::resource_offer(
        "owner/creche",
        conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
        1,
    )
}
#[test]
fn idle_attachment_preserves_biography_and_worker_returns_same_advertisement() {
    let (mut owner, _) = owner();
    let before = owner.truth();
    let advertised = owner.host.advertisement().clone();
    owner.attach_creche_encounter(slot()).unwrap();
    assert_eq!(owner.truth()["biography"], before["biography"]);
    assert_eq!(owner.truth()["realization"], before["realization"]);
    assert_eq!(owner.host.advertisement().host_id, advertised.host_id);
    assert_eq!(owner.host.advertisement().boot_id, advertised.boot_id);
    assert_eq!(
        owner.host.advertisement().offer_generation.0,
        advertised.offer_generation.0 + 1
    );
    let attached = owner.host.advertisement().clone();
    let host = owner.host.take_for_play().unwrap();
    assert_eq!(owner.host.advertisement(), &attached);
    owner.host.restore_after_play(host).unwrap();
    assert_eq!(owner.host.advertisement(), &attached);
    assert_eq!(owner.truth()["biography"], before["biography"]);
}
#[test]
fn proposal_or_worker_custody_refuses_without_changing_body_or_host() {
    let (mut owner, checked) = owner();
    owner.plan(&checked).unwrap();
    let before = owner.truth();
    assert_eq!(
        owner.attach_creche_encounter(slot()),
        Err(OwnerCrecheAttachmentRefusal::BodyActive)
    );
    assert_eq!(owner.truth(), before);
    let (mut owner, _) = self::owner();
    let host = owner.host.take_for_play().unwrap();
    let before = owner.truth();
    assert_eq!(
        owner.attach_creche_encounter(slot()),
        Err(OwnerCrecheAttachmentRefusal::BodyActive)
    );
    assert_eq!(owner.truth(), before);
    owner.host.restore_after_play(host).unwrap();
}
#[test]
fn sdk_refusal_preserves_owner_cache_and_biography() {
    let (mut owner, _) = owner();
    let before = owner.truth();
    let invalid = conduit_core::resource_offer("owner/creche", "wrong/class", 1);
    assert_eq!(
        owner.attach_creche_encounter(invalid),
        Err(OwnerCrecheAttachmentRefusal::Host(
            CrecheAttachmentRefusal::InvalidSlot
        ))
    );
    assert_eq!(owner.truth(), before);
    owner.attach_creche_encounter(slot()).unwrap();
    let before = owner.truth();
    assert_eq!(
        owner.attach_creche_encounter(slot()),
        Err(OwnerCrecheAttachmentRefusal::Host(
            CrecheAttachmentRefusal::ExistingEncounter
        ))
    );
    assert_eq!(owner.truth(), before);
}
