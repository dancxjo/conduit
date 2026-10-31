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

/// Execute the exact dispatched HTTP request under its already admitted lease.
/// The initialized endpoint/provider and actual current registry remain native
/// authority; request metadata cannot construct this admitted witness.
pub fn exchange_native_work_response(
    host: &crate::StdHost,
    work: &mut crate::PreparedNativeWork,
    provider: &IsolatedHttpBaseConfig,
) -> Result<conduit_web::HttpResponse, String> {
    host.begin_native_work(work)?;
    let selected = work.selected();
    let operation = selected
        .lowered
        .host_calls
        .iter()
        .find(|operation| {
            operation.node == work.request().node && operation.call == work.request().call
        })
        .ok_or("native HTTP operation missing")?;
    if operation.contract_id.as_str() != super::HTTP_CLIENT_OPERATION
        || work.input().len() > conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize
        || work.maximum_output_bytes() != conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES
    {
        return Err("admitted HTTP request differs from its exact finite Back contract".into());
    }
    let placement = selected
        .fragment
        .placements
        .iter()
        .find(|gear| &gear.placement_id == selected.placement_id)
        .ok_or("admitted HTTP placement missing")?;
    if placement.implementation_id.as_str() != super::ISOLATED_HTTP_IMPLEMENTATION
        || placement.kind_id.as_str() != conduit_web::HTTP_CLIENT_KIND
        || placement.host_calls.len() != 1
        || placement.authority.len() != 1
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != super::HTTP_CLIENT_RESOURCE
        || placement.host_calls[0].maximum_input_bytes
            != conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
        || placement.host_calls[0].maximum_output_bytes
            != conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES
    {
        return Err("admitted HTTP placement is not the exact installed Back".into());
    }
    let binding = placement
        .base
        .as_ref()
        .ok_or("admitted HTTP Base missing")?;
    let installed = host
        .base_registry
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
        .ok_or("admitted HTTP Base is not the actual retained provider")?;
    let decoded = conduit_web::decode_request(work.input())
        .map_err(|_| "admitted HTTP input is not its canonical request")?;
    if decoded.target.scheme() != &conduit_web::HttpScheme::Http
        || decoded.target.authority() != &provider.authority
    {
        return Err("admitted HTTP input exceeds the initialized exact endpoint".into());
    }
    let (issue, claim) = capability_for_installed_base(
        selected.fragment,
        placement,
        selected.active,
        provider,
        installed.implementation_id.as_str(),
    )?;
    // AdmittedRemoteFragment still owns the exact HTTP pool lease. Reserving
    // again would manufacture duplicate capacity or wrongly refuse its owner.
    let response = exchange_provider(provider, issue, claim, &decoded)?;
    let bytes = conduit_web::encode_response(&response)
        .map_err(|_| "admitted HTTP response exceeds canonical bounds")?;
    if bytes.len() > work.maximum_output_bytes() as usize {
        return Err("admitted HTTP response exceeds the selected output reservation".into());
    }
    Ok(response)
}
