//! Same-socket negotiation after original preparation; no application delivery.
use super::*;
/// Both variants retain the original descriptor through this serving Root.
/// Failure never silently closes a transport or pretends remote retirement.
pub(in crate::durable_host::source_revision) enum RetainedForeClient {
    Pending { _original: Box<SelectedPlayCarrier> },
    Fore { _original: Box<OwnedForeTransport> },
}
pub(in crate::durable_host::source_revision) struct OwnedForeTransport {
    _member: owner::MemberPresentationBasis,
    _transport: conduit_std_host::pico_spawn::PicoForeCarriage,
}
pub(in crate::durable_host::source_revision) fn negotiate_selected_fore(
    mut carrier: SelectedPlayCarrier,
    selected: &UsbPlaySelection,
) -> RetainedForeClient {
    if !carrier.fore_input_reported {
        return RetainedForeClient::Pending {
            _original: Box::new(carrier),
        };
    }
    let Some(prepared) = carrier.prepared_fore.take() else {
        return RetainedForeClient::Pending {
            _original: Box::new(carrier),
        };
    };
    let SelectedPlayCarrier { member, socket, .. } = carrier;
    let mut transport = socket.into_fore_carriage(prepared);
    let mut scratch = [0; conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES];
    let timeout = std::time::Duration::from_millis(selected.wait_milliseconds);
    let result = transport.handshake(&mut scratch, timeout);
    let report = serde_json::json!({
        "schema":"conduit.diagnostic/fore-session-negotiation@1",
        "member_host":member.host_advertisement.host_id,
        "member_boot":member.host_advertisement.boot_id,
        "protocol_active":transport.is_active(),
        "reported":format!("{result:?}"),
        "frames_used":transport.frames_used(),
        "admission_proved":false,
        "play_proved":false,
        "application_delivered":false,
        "recording_committed":false,
    });
    if let Err(error) =
        write_json_atomic(&selected.report_output.with_extension("line.json"), &report)
    {
        eprintln!(
            "native Fore negotiation report unavailable; original transport retained: {error}"
        );
    }
    if let Err(error) = result {
        eprintln!("native Fore negotiation refused or uncertain; local retirement only, original transport retained: {error:?}");
    }
    RetainedForeClient::Fore {
        _original: Box::new(OwnedForeTransport {
            _member: member,
            _transport: transport,
        }),
    }
}
