//! Authorized listener preparation and finite two-endpoint orchestration.
use super::*;
use conduit_std_host::secure_websocket::SecureWebSocketListener;
use std::net::SocketAddr;

pub(crate) fn serve(options: ServeOptions) -> Result<(), String> {
    serve_with_ready(options, |_| {})
}

/// Announce the exact listener only after authorization, slot and TLS admission.
/// The observer cannot consume an endpoint or alter the two-endpoint service.
pub(crate) fn serve_with_ready(
    options: ServeOptions,
    ready: impl FnOnce(SocketAddr),
) -> Result<(), String> {
    if !options.authorize_network {
        return Err("relay network listening requires --authorize-network".into());
    }
    let bind = options
        .bind
        .parse::<SocketAddr>()
        .map_err(|_| "relay --bind must be one exact socket address".to_string())?;
    if bind.ip().is_loopback() || bind.port() == 0 {
        return Err("relay --bind must be one explicit non-loopback socket".into());
    }
    if !options.public_url.starts_with("wss://") || options.public_url.len() > 256 {
        return Err("relay --public-url must be one bounded wss URL".into());
    }
    let mut slot = read_slot(&options.slot)?;
    let mut first_capability = take_capability(&mut slot.first_capability)?;
    let second_capability = match take_capability(&mut slot.second_capability) {
        Ok(capability) => capability,
        Err(error) => {
            first_capability.fill(0);
            return Err(error);
        }
    };
    let limits = RelayServiceLimits {
        maximum_slots: 1,
        maximum_protected_frame_bytes: slot.limits.maximum_protected_frame_bytes,
        maximum_queued_frames_per_direction: slot.limits.maximum_queued_frames_per_direction,
        maximum_queued_bytes_per_direction: slot.limits.maximum_queued_bytes_per_direction,
        maximum_attachment_attempts_per_slot: slot.limits.maximum_attachment_attempts_per_slot,
        maximum_idle_millis: slot.limits.maximum_idle_millis,
        maximum_active_millis: slot.limits.maximum_active_millis,
    };
    let descriptor = RelaySlotDescriptor::new(
        slot.route_id.clone(),
        slot.negotiation_id,
        slot.first_endpoint_binding,
        slot.second_endpoint_binding,
        slot.expires_at_millis,
        first_capability,
        second_capability,
    )
    .map_err(debug("validate relay slot"))?;
    let now = now_millis()?;
    let mut service = OpaqueRelayService::new(limits).map_err(debug("validate relay limits"))?;
    service
        .install_slot(descriptor, now)
        .map_err(debug("install relay slot"))?;

    let maximum_outer_bytes = usize::try_from(limits.maximum_protected_frame_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_add(256))
        .ok_or_else(|| "relay frame bound overflowed".to_string())?
        .max(MAXIMUM_CONTROL_BYTES);
    let listener = SecureWebSocketListener::bind(
        bind,
        &options.tls_cert,
        &options.tls_key,
        maximum_outer_bytes as u32,
        options.authorize_network,
    )
    .map_err(debug("bind relay WSS listener"))?;
    if listener.local_addr().map_err(debug("read relay bind"))? != bind {
        return Err("relay did not bind the exact authorized socket".into());
    }
    println!(
        "Relay ready: {} route={} implementation={} tls-sha256={}",
        options.public_url,
        slot.route_id,
        RELAY_SERVICE_IMPLEMENTATION_ID,
        hex(&listener.certificate_binding_sha256()),
    );
    println!("Capabilities remain only in the private slot file and endpoint descriptors.");
    ready(bind);

    let shared = Arc::new(Mutex::new(RelayRuntime {
        service,
        terminal: None,
    }));
    let timeout = Duration::from_secs(options.accept_timeout_seconds);
    std::thread::scope(|scope| -> Result<(), String> {
        let first = listener
            .accept_with_timeout(timeout)
            .map_err(debug("accept first relay endpoint"))?;
        let first_service = Arc::clone(&shared);
        let first_thread = scope.spawn(move || serve_endpoint(first, first_service, 1, limits));
        let second = listener
            .accept_with_timeout(timeout)
            .map_err(debug("accept second relay endpoint"))?;
        let second_service = Arc::clone(&shared);
        let second_thread = scope.spawn(move || serve_endpoint(second, second_service, 2, limits));
        let first_result = first_thread
            .join()
            .map_err(|_| "first relay endpoint worker panicked".to_string())?;
        let second_result = second_thread
            .join()
            .map_err(|_| "second relay endpoint worker panicked".to_string())?;
        first_result.and(second_result)
    })
}
