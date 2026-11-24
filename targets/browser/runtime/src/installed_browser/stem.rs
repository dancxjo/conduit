//! Exact browser Back; only a private Root provision can initialize it.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserBack,
};
#[cfg(feature = "creche-surface")]
use conduit_body_make::stem_runtime::StemBack;
use conduit_body_make::stem_runtime_catalog::*;
use conduit_core::*;
use conduit_kernel::HostedValueStore;

pub(crate) const IMPLEMENTATION: &str = "browser/stem-prepare@1";
pub(crate) const RESOURCE: &str = "conduit.resource/stem-provision@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        stem_kind(),
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: "browser/stem-kernel-hosted@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: "conduit-browser-runtime/stem-prepare@1".into(),
            host_calls: vec![stem_host_call_requirement()],
            resource_requirements: vec![ResourceRequirement {
                class_id: RESOURCE.into(),
                units: 1,
                protected_role: None,
                compute: None,
                content: None,
            }],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub(crate) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    validate_placement(placement, &offer)?;
    if placement.capability_id != offer.capability_id
        || placement.limits != offer.limits
        || placement.base.is_some()
        || !placement.configuration.is_empty()
        || !placement.realization_characteristics.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || placement.resources.len() != 1
    {
        return Err("Stem placement differs from its exact browser Back".into());
    }
    let resource = &placement.resources[0];
    if resource.class_id.as_str() != RESOURCE
        || resource.units != 1
        || resource.pool_id.as_str().is_empty()
        || resource.protected.is_some()
        || resource.compute.is_some()
        || resource.content.is_some()
    {
        return Err("Stem requires one exact private provision resource".into());
    }
    Ok(())
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    validate(placement)?;
    #[cfg(feature = "creche-surface")]
    {
        crate::workspace_runtime::stem_provision::bind(placement)?;
        Ok(BrowserBack::installed_step(StemBack::for_admitted_host(
            conduit_kernel::HostCallId(0),
        )))
    }
    #[cfg(not(feature = "creche-surface"))]
    Err("Stem requires the private Body issuer owner".into())
}
