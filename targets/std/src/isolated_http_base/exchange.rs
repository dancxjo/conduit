//! Existing exact-endpoint exchange on an already installed retained native Base.
use super::host::{capability_for_installed_base, exact_http_placement, exchange_provider};
use super::{IsolatedHttpBaseConfig, IsolatedHttpExchangeReceipt};
/// The selected current Base must already belong to this actual Host inventory.
/// Its HTTP resource reservation nests independently of a caller Source lease.
pub fn exchange_selected_response(
    host: &mut crate::StdHost,
    provider: &IsolatedHttpBaseConfig,
    play: crate::IssuedKernelPlay,
    fragment: &conduit_core::PlanFragment,
    request: &conduit_web::HttpRequest,
) -> Result<(IsolatedHttpExchangeReceipt, conduit_web::HttpResponse), String> {
    let registry = host.base_registry.clone();
    exchange_with_registry(host, &registry, provider, play, fragment, request)
}
pub(super) fn exchange_with_registry(
    host: &mut crate::StdHost,
    registry: &conduit_core::BaseRegistry,
    provider: &IsolatedHttpBaseConfig,
    play: crate::IssuedKernelPlay,
    fragment: &conduit_core::PlanFragment,
    request: &conduit_web::HttpRequest,
) -> Result<(IsolatedHttpExchangeReceipt, conduit_web::HttpResponse), String> {
    let active_play = play.identity();
    if active_play.plan_id != fragment.plan_id
        || active_play.host_id != fragment.host_id
        || active_play.boot_id != fragment.boot_id
    {
        return Err("HTTP Play identity does not match its immutable Plan fragment".into());
    }
    request
        .validate()
        .map_err(|_| "isolated HTTP request exceeds its semantic bounds".to_string())?;
    if request.target.scheme() != &conduit_web::HttpScheme::Http
        || request.target.authority() != &provider.authority
    {
        return Err("isolated HTTP request is outside the selected exact endpoint".into());
    }
    let placement = exact_http_placement(fragment)?;
    let binding = placement.base.as_ref().ok_or("HTTP Base binding missing")?;
    let installed = registry
        .entries()
        .iter()
        .find(|entry| {
            entry.base_id == binding.base_id
                && entry.provider_instance_id == binding.provider_instance_id
                && entry.provider_generation == binding.provider_generation
                && entry.implementation_id == binding.implementation_id
                && entry.mechanism_family == binding.mechanism_family
                && entry.enforcement_class == binding.enforcement_class
                && entry.lifecycle == conduit_core::BaseLifecycle::Ready
                && entry
                    .capabilities
                    .iter()
                    .any(|offer| offer.capability_id == placement.capability_id)
                && entry
                    .resources
                    .iter()
                    .any(|resource| resource.pool_id == placement.resources[0].pool_id)
        })
        .ok_or("HTTP Base is not the actual installed native provider")?;
    let (issue, claim) = capability_for_installed_base(
        fragment,
        placement,
        active_play,
        provider,
        installed.implementation_id.as_str(),
    )?;
    let reservation = host
        .kernel_resources
        .prepare_and_reserve(&host.advertisement, fragment)?;
    let result = exchange_provider(provider, issue, claim, request);
    let release = host.kernel_resources.release(reservation);
    release?;
    let response = result?;
    Ok((
        IsolatedHttpExchangeReceipt {
            active_play_id: active_play.active_play_id.clone(),
            status: response.status,
            response_bytes: conduit_web::encode_response(&response)
                .map_err(|_| "isolated HTTP response exceeds semantic bounds")?
                .len() as u32,
        },
        response,
    ))
}
