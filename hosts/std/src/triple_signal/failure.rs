use std::io::Write;
use std::time::Duration;

use conduit_wire::{SessionMessage, SessionTerminalDisposition};

use crate::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};

use super::{RemoteKind, TriplePhysicalRunner, FRAME_BYTES};
use crate::triple_signal::PicoRuntimeIdentity;

impl TriplePhysicalRunner {
    pub(super) fn fail_pico_branch<W: Write>(
        &mut self,
        carrier: &mut NativePathCdcCarrier,
        evidence: &mut NativePathCdcLineReader,
        runtime: &PicoRuntimeIdentity,
        code: u16,
        report: &mut W,
    ) -> Result<(), String> {
        self.source.cancel()?;
        let final_sequence = self.source.next_sequence(RemoteKind::Pico);
        let binding = self.source.binding(RemoteKind::Pico).clone();
        let failed = binding.frame(SessionMessage::Failed { code });
        self.source.admit_outbound(RemoteKind::Pico, failed)?;
        carrier
            .send_frame(&failed, Duration::from_secs(2))
            .map_err(|error| format!("Pico Failed send: {error:?}"))?;

        let mut bytes = [0_u8; FRAME_BYTES];
        let reciprocal = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico Failed receive: {error:?}"))?;
        self.source.admit_inbound(RemoteKind::Pico, reciprocal)?;
        if !matches!(reciprocal.message, SessionMessage::Failed { code: actual } if actual == code)
        {
            return Err(format!(
                "unexpected Pico failure response: {:?}",
                reciprocal.message
            ));
        }

        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Failed,
            final_sequence,
        });
        self.source.admit_outbound(RemoteKind::Pico, terminal)?;
        carrier
            .send_frame(&terminal, Duration::from_secs(2))
            .map_err(|error| format!("Pico failed terminal send: {error:?}"))?;
        let reciprocal = carrier
            .receive_frame(&mut bytes, Duration::from_secs(3))
            .map_err(|error| format!("Pico failed terminal receive: {error:?}"))?;
        self.source.admit_inbound(RemoteKind::Pico, reciprocal)?;
        if !matches!(
            reciprocal.message,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Failed,
                final_sequence: actual,
            } if actual == final_sequence
        ) || !self.source.is_terminal(RemoteKind::Pico)
        {
            return Err(format!(
                "unexpected Pico failed terminal response: {:?}",
                reciprocal.message
            ));
        }

        let terminal_evidence = evidence
            .read_line(Duration::from_secs(3))
            .map_err(|error| format!("Pico failed terminal evidence: {error:?}"))?;
        self.pico_evidence
            .verify_terminal(&terminal_evidence, runtime, false)?;
        writeln!(
            report,
            "summary plan={} pico_link={} pico_boot={} values={} terminal=failed failure_code={code}",
            self.source.fragment().plan_id.as_str(),
            binding.attachment.link_binding_id.as_str(),
            runtime.boot_id,
            final_sequence,
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }
}
