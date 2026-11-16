//! Same-socket negotiation after original preparation; no application delivery.
use super::*;
pub(crate) use fore_provider::ForeProbeBinding;
use std::time::Duration;

/// One synchronous authorized diagnostic probe, not a Started or Ready handle.
/// The sole transport and original binding are borrowed only for this callback.
#[allow(dead_code)] // Consumer hook is independently integrated by the Root lane.
pub(crate) struct AuthorizedForeProbe<'a> {
    binding: ForeProbeBinding<'a>,
    transport: &'a mut conduit_std_host::pico_spawn::PicoForeCarriage,
    maximum_timeout: Duration,
    attempted: bool,
}
#[allow(dead_code)]
impl AuthorizedForeProbe<'_> {
    pub(crate) fn binding(&self) -> &ForeProbeBinding<'_> {
        &self.binding
    }
    /// Transport ACK is not durable recording acknowledgment. The consumer
    /// validates/correlates application bytes before the existing result ACK.
    pub(crate) fn exchange(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        timeout: Duration,
        consume: impl FnOnce(&[u8]) -> Result<(), ()>,
    ) -> Result<
        conduit_std_host::pico_spawn::ForeExchangeOutcome,
        conduit_std_host::pico_spawn::ForeCarriageError,
    > {
        if self.attempted || timeout.is_zero() || timeout > self.maximum_timeout {
            return Err(conduit_std_host::pico_spawn::ForeCarriageError::Bounds);
        }
        // Exactly one attempted exchange per authorized borrow, no retry.
        self.attempted = true;
        self.transport.exchange(input, output, timeout, consume)
    }
}

/// Both variants retain the original descriptor through this serving Root.
/// Failure never silently closes a transport or pretends remote retirement.
pub(in crate::durable_host::source_revision) enum RetainedForeClient {
    Pending { _original: Box<SelectedPlayCarrier> },
    Fore { _original: Box<OwnedForeTransport> },
}
pub(in crate::durable_host::source_revision) struct OwnedForeTransport {
    member: owner::MemberPresentationBasis,
    transport: conduit_std_host::pico_spawn::PicoForeCarriage,
    material: fore_provider::DirectForeMaterial,
    negotiated: bool,
    maximum_timeout: Duration,
}
impl RetainedForeClient {
    pub(in crate::durable_host::source_revision) fn matches_host(
        &self,
        host: &conduit_core::HostId,
    ) -> bool {
        match self {
            Self::Pending { _original } => &_original.member.host_advertisement.host_id == host,
            Self::Fore { _original } => &_original.member.host_advertisement.host_id == host,
        }
    }
    /// Verification is read-only; merely selecting this borrow performs no IO.
    pub(in crate::durable_host::source_revision) fn probe<R>(
        &mut self,
        owner: &owner::Owner,
        operation: impl for<'probe> FnOnce(&mut AuthorizedForeProbe<'probe>) -> Result<R, String>,
    ) -> Result<R, String> {
        let Self::Fore {
            _original: original,
        } = self
        else {
            return Err(
                "Fore probe requires original reported input and negotiated transport".into(),
            );
        };
        if !original.negotiated || !original.transport.is_active() {
            return Err("Fore probe negotiation is absent, refused or retired".into());
        }
        let transcript = original.material.approval_transcript()?;
        // This hook returns ONLY the actual Owner's still-retained command,
        // after current Root/member/Body/Source/Plan checks. No report is proof.
        let command = owner.verify_selected_fore_probe(&original.member, transcript)?;
        let binding = original
            .material
            .verify_probe_binding(command.payload(), command.transcript())?;
        let mut probe = AuthorizedForeProbe {
            binding,
            transport: &mut original.transport,
            maximum_timeout: original.maximum_timeout,
            attempted: false,
        };
        operation(&mut probe)
    }
}
pub(in crate::durable_host::source_revision) fn negotiate_selected_fore(
    mut carrier: SelectedPlayCarrier,
    selected: &UsbPlaySelection,
) -> RetainedForeClient {
    if !carrier.fore_input_reported
        || !(1..=10_000).contains(&selected.wait_milliseconds)
        || carrier
            .fore_material
            .as_ref()
            .is_none_or(|material| material.approval_transcript().is_err())
    {
        return RetainedForeClient::Pending {
            _original: Box::new(carrier),
        };
    }
    let Some(prepared) = carrier.prepared_fore.take() else {
        return RetainedForeClient::Pending {
            _original: Box::new(carrier),
        };
    };
    let Some(material) = carrier.fore_material.take() else {
        return RetainedForeClient::Pending {
            _original: Box::new(carrier),
        };
    };
    let SelectedPlayCarrier { member, socket, .. } = carrier;
    let mut transport = socket.into_fore_carriage(prepared);
    let mut scratch = [0; conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES];
    let timeout = std::time::Duration::from_millis(selected.wait_milliseconds);
    let result = transport.handshake(&mut scratch, timeout);
    let negotiated = result.is_ok() && transport.is_active();
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
            member,
            transport,
            material,
            negotiated,
            maximum_timeout: timeout,
        }),
    }
}
