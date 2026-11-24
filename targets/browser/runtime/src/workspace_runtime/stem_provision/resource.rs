//! Observation of one actually retained private preparation resource.
use super::*;
pub(in crate::workspace_runtime) fn observe(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    id: [u8; 32],
    placement: conduit_core::PlannedGear,
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, host, boot)?;
    PROVISION.with(|slot| {
        let slot=slot.borrow();
        let provision=slot.as_ref().ok_or("Stem resource owner is absent")?;
        if provision.id!=id || provision.host!=*host || provision.boot!=*boot || provision.provider_attempted || provision.target_job_pending || provision.delivery_committed {
            return Err(Refusal::new("Stem.StaleResource","Stem preparation owner is foreign or retired"));
        }
        let actual=body.realization().ok_or("Stem resource requires an actual proposal")?;
        if actual.play.is_some() {return Err(Refusal::new("Stem.StaleResource","New preparation observations are unavailable during Play"));}
        actual.plan.validate_for(&actual.wake).map_err(|error|Refusal::new("Stem.StalePlan",format!("{error:?}")))?;
        let selected=actual.plan.plots.iter().flat_map(|plot|&plot.plan.fragments).filter(|fragment|fragment.host_id==*host && fragment.boot_id==*boot)
            .flat_map(|fragment|&fragment.placements).any(|gear|gear==&placement);
        let resource=provision.resource();
        if !selected || placement.host_id!=*host || placement.boot_id!=*boot || placement.kind_id.as_str()!=conduit_body_make::stem_runtime::STEM_KIND
            || placement.resources.len()!=1 || placement.resources[0].pool_id!=resource.pool_id || placement.resources[0].class_id!=resource.class_id || placement.resources[0].units!=1 {
            return Err(Refusal::new("Stem.StaleResource","Observation differs from exact proposed Stem pool/placement"));
        }
        encode(&serde_json::json!({"host_id":host,"boot_id":boot,"offer_generation":placement.offer_generation,
            "pool_id":resource.pool_id,"class_id":resource.class_id,"health":"Ready","unreserved_units":1,"utilized_units":0,
            "sign_id":format!("stem-resource/{}",hex_id(&provision.id))}))
    })
}
fn hex_id(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
