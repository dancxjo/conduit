//! One-packet USB2 Isoch TD encoding and exact event correlation.
use super::Event;
use crate::usb_base::isochronous_schedule::{IsochronousFrame, IsochronousOutObservation};
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IsochronousEventRefusal {
    Foreign,
    Residual,
    Completion(u8),
}
/// xHCI 1.2b §6.4.1.3: type 5, explicit Frame ID, IOC. One packet means
/// TBC=0 and TLBPC=0, final TD Size=0, and SIA=0. No Normal-TRB substitution.
pub(crate) fn out_trb(buffer: u64, length: u16, cycle: u32, frame: IsochronousFrame) -> [u32; 4] {
    [
        buffer as u32,
        (buffer >> 32) as u32,
        u32::from(length),
        (5 << 10) | (1 << 5) | cycle | (u32::from(frame.number()) << 20),
    ]
}
pub(crate) fn observation(
    event: Event,
    slot: u8,
    dci: u8,
    pointer: u64,
    length: u16,
    frame: u16,
) -> Result<IsochronousOutObservation, IsochronousEventRefusal> {
    if event.event_type != 32 || event.slot != slot || event.endpoint != dci {
        return Err(IsochronousEventRefusal::Foreign);
    }
    // Ring Underrun is endpoint scoped, not a completion of the pending TD.
    if event.completion_code == 14 {
        return Ok(IsochronousOutObservation::Underrun);
    }
    if event.pointer != pointer {
        return Err(IsochronousEventRefusal::Foreign);
    }
    let residual = u16::try_from(event.residual).map_err(|_| IsochronousEventRefusal::Residual)?;
    let actual = length
        .checked_sub(residual)
        .ok_or(IsochronousEventRefusal::Residual)?;
    match event.completion_code {
        1 if actual == length => Ok(IsochronousOutObservation::Completed {
            frame,
            requested: length,
            actual,
        }),
        1 | 13 => Ok(IsochronousOutObservation::Short {
            frame,
            requested: length,
            actual,
        }),
        23 => Ok(IsochronousOutObservation::MissedService { frame }),
        code => Err(IsochronousEventRefusal::Completion(code)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isoch_trb_has_real_type_and_explicit_frame() {
        let trb = out_trb(
            0x1234_5678_9000,
            192,
            1,
            IsochronousFrame::new(2047).unwrap(),
        );
        assert_eq!(
            trb,
            [0x5678_9000, 0x1234, 192, (2047 << 20) | (5 << 10) | 33]
        );
        assert_eq!(trb[3] >> 31, 0);
    }
    #[test]
    fn underrun_never_becomes_a_transfer_completion() {
        let event = Event {
            event_type: 32,
            completion_code: 14,
            slot: 1,
            endpoint: 2,
            residual: 0,
            pointer: 0,
        };
        assert_eq!(
            observation(event, 1, 2, 4096, 192, 8),
            Ok(IsochronousOutObservation::Underrun)
        );
        assert_eq!(
            observation(event, 2, 2, 4096, 192, 8),
            Err(IsochronousEventRefusal::Foreign)
        );
    }
    #[test]
    fn exact_completion_short_and_missed_service_are_distinct() {
        let mut event = Event {
            event_type: 32,
            completion_code: 1,
            slot: 1,
            endpoint: 2,
            residual: 0,
            pointer: 4096,
        };
        assert!(matches!(
            observation(event, 1, 2, 4096, 192, 8),
            Ok(IsochronousOutObservation::Completed { actual: 192, .. })
        ));
        event.residual = 4;
        event.completion_code = 13;
        assert!(matches!(
            observation(event, 1, 2, 4096, 192, 8),
            Ok(IsochronousOutObservation::Short { actual: 188, .. })
        ));
        event.completion_code = 23;
        assert_eq!(
            observation(event, 1, 2, 4096, 192, 8),
            Ok(IsochronousOutObservation::MissedService { frame: 8 })
        );
        event.pointer = 4112;
        assert_eq!(
            observation(event, 1, 2, 4096, 192, 8),
            Err(IsochronousEventRefusal::Foreign)
        );
    }
}
