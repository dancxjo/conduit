use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use conduit_signal::{decode_signal_bytes, DISTRIBUTED_MAXIMUM_FRAME_BYTES, SIGNAL_ENCODED_LEN};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionMessage, SessionTerminalDisposition,
};

use crate::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};
use crate::websocket::{NativeWebSocketCarrier, NativeWebSocketListener};

use super::{PicoEvidence, RemoteKind, TripleSource};

#[path = "failure.rs"]
mod failure;

const FRAME_BYTES: usize = DISTRIBUTED_MAXIMUM_FRAME_BYTES as usize;

pub fn default_pico_ports() -> Result<(PathBuf, PathBuf), String> {
    let base = Path::new("/dev/serial/by-id");
    let entries = std::fs::read_dir(base)
        .map_err(|error| format!("cannot inspect {}: {error}", base.display()))?;
    let mut link = None;
    let mut evidence = None;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !name.contains("Conduit_Pico_W_Signal") {
            continue;
        }
        if name.ends_with("-if00") {
            link = Some(path);
        } else if name.ends_with("-if02") {
            evidence = Some(path);
        }
    }
    Ok((
        link.ok_or_else(|| "Pico CDC0 link interface is not present".to_owned())?,
        evidence.ok_or_else(|| "Pico CDC1 evidence interface is not present".to_owned())?,
    ))
}

pub struct TriplePhysicalRunner {
    pub(super) source: TripleSource,
    pub(super) pico_evidence: PicoEvidence,
}

impl TriplePhysicalRunner {
    pub fn prepare() -> Result<Self, String> {
        Ok(Self {
            source: TripleSource::prepare()?,
            pico_evidence: PicoEvidence::exact_triple()?,
        })
    }

    pub fn run<W: Write>(
        mut self,
        listener: NativeWebSocketListener,
        link_path: &Path,
        evidence_path: &Path,
        report: &mut W,
    ) -> Result<(), String> {
        let mut evidence = NativePathCdcLineReader::open(evidence_path)
            .map_err(|error| format!("open Pico CDC1: {error:?}"))?;
        let mut pico = NativePathCdcCarrier::open(link_path, 1_024)
            .map_err(|error| format!("open Pico CDC0: {error:?}"))?;
        std::thread::sleep(Duration::from_millis(250));
        pico.send_raw_stream_frame(b"CONDUIT_RAW_CDC0_PROBE", Duration::from_secs(2))
            .map_err(|error| format!("Pico raw probe send: {error:?}"))?;
        let mut pico_bytes = [0_u8; FRAME_BYTES];
        let reply = pico
            .receive_raw_stream_frame(&mut pico_bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico raw probe receive: {error:?}"))?;
        if reply != b"CONDUIT_RAW_CDC0_REPLY" {
            return Err("Pico raw CDC0 probe reply mismatch".to_owned());
        }
        let boot_line = evidence
            .read_line(Duration::from_secs(3))
            .map_err(|error| format!("Pico boot evidence: {error:?}"))?;
        let runtime = self.pico_evidence.verify_boot(&boot_line)?;
        let ready = evidence
            .read_line(Duration::from_secs(10))
            .map_err(|error| format!("Pico GPIO readiness: {error:?}"))?;
        if ready != "CONDUIT_CYW43_GPIO_READY" {
            return Err(format!("unexpected Pico readiness record: {ready}"));
        }
        self.source
            .observe_pico_boot(conduit_core::BootId::from(runtime.boot_id.as_str()))?;

        let mut browser = listener.accept().map_err(|error| format!("{error:?}"))?;
        self.activate_browser(&mut browser)?;
        self.activate_pico(&mut pico)?;
        if !self.source.is_active(RemoteKind::Browser) || !self.source.is_active(RemoteKind::Pico) {
            return Err(
                "both exact remote sessions were not active before source execution".into(),
            );
        }

        let execution = (|| -> Result<u64, String> {
            let mut observed = 0_u64;
            while let Some(offer) = self.source.next_offer()? {
                if offer.sequence != observed {
                    return Err("triple source sequence is not contiguous".to_owned());
                }
                self.offer_browser(&mut browser, &offer)?;
                self.offer_pico(&mut pico, &offer)?;
                self.await_browser_delivery(&mut browser, offer.sequence)?;

                let signal =
                    decode_signal_bytes(&offer.payload).map_err(|error| error.to_string())?;
                let receipt_line = evidence
                    .read_line(Duration::from_secs(3))
                    .map_err(|error| format!("Pico receipt {}: {error:?}", offer.sequence))?;
                self.pico_evidence.verify_receipt(
                    &receipt_line,
                    &runtime,
                    signal.sequence,
                    signal.level,
                )?;
                self.await_pico_delivery(&mut pico, offer.sequence)?;

                let stdout = self.source.manifest_stdout(offer.sequence)?;
                if stdout.sequence != signal.sequence || stdout.level != signal.level {
                    return Err("stdout receipt disagrees with remote Signal value".to_owned());
                }
                writeln!(
                    report,
                    "{}",
                    serde_json::json!({
                        "schema": "conduit-three-host/stdout-receipt@1",
                        "plan_id": stdout.plan_id,
                        "fragment_id": stdout.fragment_id,
                        "active_play_id": stdout.active_play_id,
                        "placement_id": stdout.placement_id.as_str(),
                        "presentation_id": stdout.presentation_id.as_str(),
                        "evidence_id": stdout.evidence_id.as_str(),
                        "sequence": stdout.sequence,
                        "level": stdout.level,
                    })
                )
                .map_err(|error| error.to_string())?;
                observed += 1;
            }

            let final_sequence = self.source.finish_kernel()?;
            if observed != 16 || final_sequence != observed {
                return Err("three-host terminal count is not sixteen".to_owned());
            }
            self.complete_browser(&mut browser, final_sequence)?;
            self.complete_pico(&mut pico, final_sequence)?;
            let terminal = evidence
                .read_line(Duration::from_secs(3))
                .map_err(|error| format!("Pico terminal evidence: {error:?}"))?;
            self.pico_evidence
                .verify_terminal(&terminal, &runtime, true)?;
            if !self.source.is_terminal(RemoteKind::Browser)
                || !self.source.is_terminal(RemoteKind::Pico)
            {
                return Err("three-host sessions lack reciprocal terminal agreement".into());
            }
            browser.close().map_err(|error| format!("{error:?}"))?;
            Ok(observed)
        })();
        match execution {
            Ok(_) => {}
            Err(cause) => {
                let propagation =
                    self.fail_pico_branch(&mut pico, &mut evidence, &runtime, 350, report);
                return Err(match propagation {
                    Ok(()) => format!("{cause}; failure propagated to Pico terminal"),
                    Err(error) => format!("{cause}; Pico failure propagation: {error}"),
                });
            }
        }
        writeln!(
            report,
            "summary plan={} source_fragment={} browser_fragment={} pico_fragment={} browser_link={} pico_link={} pico_boot={} pico_play={} firmware_build={} values=16 stdout_receipts=16 browser_receipts=16 pico_receipts=16 terminal=completed",
            self.source.fragment().plan_id.as_str(),
            self.source.fragment().fragment_id.as_str(),
            self.source.binding(RemoteKind::Browser).sink_fragment_id.as_str(),
            self.source.binding(RemoteKind::Pico).sink_fragment_id.as_str(),
            self.source.binding(RemoteKind::Browser).link_binding_id.as_str(),
            self.source.binding(RemoteKind::Pico).link_binding_id.as_str(),
            runtime.boot_id,
            runtime.active_play_id,
            self.pico_evidence.firmware_build_id().unwrap_or("missing"),
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn activate_browser(&mut self, carrier: &mut NativeWebSocketCarrier) -> Result<(), String> {
        let hello = receive_browser(&mut self.source, carrier)?;
        if !matches!(hello, BrowserInbound::Hello) {
            return Err("browser did not begin with Hello".into());
        }
        let binding = self.source.binding(RemoteKind::Browser).clone();
        let hello = binding.hello_frame();
        self.source.admit_outbound(RemoteKind::Browser, hello)?;
        send_browser(carrier, hello)?;
        let ready = receive_browser(&mut self.source, carrier)?;
        if !matches!(ready, BrowserInbound::Ready) {
            return Err("browser did not report Ready".into());
        }
        let ready = binding.frame(SessionMessage::Ready);
        self.source.admit_outbound(RemoteKind::Browser, ready)?;
        send_browser(carrier, ready)
    }

    fn activate_pico(&mut self, carrier: &mut NativePathCdcCarrier) -> Result<(), String> {
        let binding = self.source.binding(RemoteKind::Pico).clone();
        let hello = binding.hello_frame();
        self.source.admit_outbound(RemoteKind::Pico, hello)?;
        carrier
            .send_frame(&hello, Duration::from_secs(2))
            .map_err(|error| format!("Pico Hello send: {error:?}"))?;
        let mut bytes = [0_u8; FRAME_BYTES];
        let hello = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico Hello receive: {error:?}"))?;
        if !matches!(hello.message, SessionMessage::Hello(_)) {
            return Err("Pico did not reciprocate Hello".into());
        }
        self.source.admit_inbound(RemoteKind::Pico, hello)?;
        let ready = binding.frame(SessionMessage::Ready);
        self.source.admit_outbound(RemoteKind::Pico, ready)?;
        carrier
            .send_frame(&ready, Duration::from_secs(2))
            .map_err(|error| format!("Pico Ready send: {error:?}"))?;
        let ready = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico Ready receive: {error:?}"))?;
        if !matches!(ready.message, SessionMessage::Ready) {
            return Err("Pico did not reciprocate Ready".into());
        }
        self.source.admit_inbound(RemoteKind::Pico, ready)
    }

    fn offer_browser(
        &mut self,
        carrier: &mut NativeWebSocketCarrier,
        offer: &super::TripleOffer,
    ) -> Result<(), String> {
        let binding = self.source.binding(RemoteKind::Browser).clone();
        loop {
            let frame = binding.frame(SessionMessage::Offered {
                sequence: offer.sequence,
                payload: &offer.payload,
            });
            self.source.admit_outbound(RemoteKind::Browser, frame)?;
            send_browser(carrier, frame)?;
            let response = receive_browser(&mut self.source, carrier)?;
            match response {
                BrowserInbound::Pressure(sequence) if sequence == offer.sequence => {
                    self.source.pressure(RemoteKind::Browser, offer.sequence)?;
                }
                BrowserInbound::Accepted(sequence) if sequence == offer.sequence => {
                    self.source.accepted(RemoteKind::Browser, offer.sequence)?;
                    return Ok(());
                }
                other => return Err(format!("unexpected browser offer response: {other:?}")),
            }
        }
    }

    fn offer_pico(
        &mut self,
        carrier: &mut NativePathCdcCarrier,
        offer: &super::TripleOffer,
    ) -> Result<(), String> {
        let binding = self.source.binding(RemoteKind::Pico).clone();
        let frame = binding.frame(SessionMessage::Offered {
            sequence: offer.sequence,
            payload: &offer.payload,
        });
        self.source.admit_outbound(RemoteKind::Pico, frame)?;
        carrier
            .send_frame(&frame, Duration::from_secs(2))
            .map_err(|error| format!("Pico Offered send: {error:?}"))?;
        let mut bytes = [0_u8; FRAME_BYTES];
        let accepted = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico Accepted receive: {error:?}"))?;
        self.source.admit_inbound(RemoteKind::Pico, accepted)?;
        match accepted.message {
            SessionMessage::Accepted { sequence } if sequence == offer.sequence => {
                self.source.accepted(RemoteKind::Pico, offer.sequence)
            }
            other => Err(format!("unexpected Pico offer response: {other:?}")),
        }
    }

    fn await_browser_delivery(
        &mut self,
        carrier: &mut NativeWebSocketCarrier,
        sequence: u64,
    ) -> Result<(), String> {
        let delivered = receive_browser(&mut self.source, carrier)?;
        match delivered {
            BrowserInbound::Delivered(actual) if actual == sequence => {
                self.source.delivered(RemoteKind::Browser, sequence)
            }
            other => Err(format!("unexpected browser delivery: {other:?}")),
        }
    }

    fn await_pico_delivery(
        &mut self,
        carrier: &mut NativePathCdcCarrier,
        sequence: u64,
    ) -> Result<(), String> {
        let mut bytes = [0_u8; FRAME_BYTES];
        let delivered = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico Delivered receive: {error:?}"))?;
        self.source.admit_inbound(RemoteKind::Pico, delivered)?;
        match delivered.message {
            SessionMessage::Delivered { sequence: actual } if actual == sequence => {
                self.source.delivered(RemoteKind::Pico, sequence)
            }
            other => Err(format!("unexpected Pico delivery: {other:?}")),
        }
    }

    fn complete_browser(
        &mut self,
        carrier: &mut NativeWebSocketCarrier,
        final_sequence: u64,
    ) -> Result<(), String> {
        let binding = self.source.binding(RemoteKind::Browser).clone();
        for message in [
            SessionMessage::InputClosed { final_sequence },
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            },
        ] {
            let frame = binding.frame(message);
            self.source.admit_outbound(RemoteKind::Browser, frame)?;
            send_browser(carrier, frame)?;
        }
        let terminal = receive_browser(&mut self.source, carrier)?;
        match terminal {
            BrowserInbound::Terminal(SessionTerminalDisposition::Completed, actual)
                if actual == final_sequence =>
            {
                Ok(())
            }
            other => Err(format!("unexpected browser terminal: {other:?}")),
        }
    }

    fn complete_pico(
        &mut self,
        carrier: &mut NativePathCdcCarrier,
        final_sequence: u64,
    ) -> Result<(), String> {
        let binding = self.source.binding(RemoteKind::Pico).clone();
        for message in [
            SessionMessage::InputClosed { final_sequence },
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            },
        ] {
            let frame = binding.frame(message);
            self.source.admit_outbound(RemoteKind::Pico, frame)?;
            carrier
                .send_frame(&frame, Duration::from_secs(2))
                .map_err(|error| format!("Pico terminal send: {error:?}"))?;
        }
        let mut bytes = [0_u8; FRAME_BYTES];
        let terminal = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico terminal receive: {error:?}"))?;
        self.source.admit_inbound(RemoteKind::Pico, terminal)?;
        match terminal.message {
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: actual,
            } if actual == final_sequence => Ok(()),
            other => Err(format!("unexpected Pico terminal: {other:?}")),
        }
    }
}

fn send_browser(
    carrier: &mut NativeWebSocketCarrier,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<(), String> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = encode_session_frame_into(
        frame,
        &mut bytes,
        SIGNAL_ENCODED_LEN,
        DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("{error:?}"))?;
    carrier
        .send_binary(&bytes[..length])
        .map_err(|error| format!("{error:?}"))
}

#[derive(Debug)]
enum BrowserInbound {
    Hello,
    Ready,
    Pressure(u64),
    Accepted(u64),
    Delivered(u64),
    Terminal(SessionTerminalDisposition, u64),
    Other,
}

fn receive_browser(
    source: &mut TripleSource,
    carrier: &mut NativeWebSocketCarrier,
) -> Result<BrowserInbound, String> {
    let mut bytes = [0_u8; FRAME_BYTES];
    let length = carrier
        .receive_binary(&mut bytes)
        .map_err(|error| format!("{error:?}"))?;
    let frame = decode_session_frame(
        &bytes[..length],
        SIGNAL_ENCODED_LEN,
        DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    )
    .map_err(|error| format!("{error:?}"))?;
    source.admit_inbound(RemoteKind::Browser, frame)?;
    Ok(match frame.message {
        SessionMessage::Hello(_) => BrowserInbound::Hello,
        SessionMessage::Ready => BrowserInbound::Ready,
        SessionMessage::Pressure { sequence } => BrowserInbound::Pressure(sequence),
        SessionMessage::Accepted { sequence } => BrowserInbound::Accepted(sequence),
        SessionMessage::Delivered { sequence } => BrowserInbound::Delivered(sequence),
        SessionMessage::Terminal {
            disposition,
            final_sequence,
        } => BrowserInbound::Terminal(disposition, final_sequence),
        _ => BrowserInbound::Other,
    })
}
