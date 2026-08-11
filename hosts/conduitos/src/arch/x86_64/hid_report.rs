//! Finite HID boot-report validation and deterministic transition derivation.

use super::{BOOT_REPORT_BYTES, HidError, HidKeyTransition, MAX_TRANSITIONS_PER_REPORT};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct BootReport {
    pub(super) modifiers: u8,
    usages: [u8; 6],
}

pub(super) fn parse_report(bytes: &[u8]) -> Result<BootReport, HidError> {
    if bytes.len() != BOOT_REPORT_BYTES {
        return Err(HidError::ShortReport);
    }
    if bytes[1] != 0 {
        return Err(HidError::ReservedByte);
    }
    let mut usages = [0u8; 6];
    usages.copy_from_slice(&bytes[2..]);
    for (index, usage) in usages.iter().copied().enumerate() {
        if (1..=3).contains(&usage) {
            return Err(HidError::Rollover);
        }
        if usage != 0 && usages[..index].contains(&usage) {
            return Err(HidError::DuplicateUsage);
        }
    }
    usages.sort_unstable();
    Ok(BootReport {
        modifiers: bytes[0],
        usages,
    })
}

pub(super) fn derive_transitions(
    previous: BootReport,
    current: BootReport,
) -> Result<([HidKeyTransition; MAX_TRANSITIONS_PER_REPORT], usize), HidError> {
    let mut output = [HidKeyTransition::default(); MAX_TRANSITIONS_PER_REPORT];
    let mut count = 0usize;
    for bit in 0..8 {
        let mask = 1 << bit;
        if previous.modifiers & mask != current.modifiers & mask {
            push(
                &mut output,
                &mut count,
                HidKeyTransition {
                    usage: 0xe0 + bit,
                    pressed: current.modifiers & mask != 0,
                    modifiers: current.modifiers,
                },
            )?;
        }
    }
    for usage in previous.usages.iter().copied().filter(|usage| *usage != 0) {
        if !current.usages.contains(&usage) {
            push(
                &mut output,
                &mut count,
                HidKeyTransition {
                    usage,
                    pressed: false,
                    modifiers: current.modifiers,
                },
            )?;
        }
    }
    for usage in current.usages.iter().copied().filter(|usage| *usage != 0) {
        if !previous.usages.contains(&usage) {
            push(
                &mut output,
                &mut count,
                HidKeyTransition {
                    usage,
                    pressed: true,
                    modifiers: current.modifiers,
                },
            )?;
        }
    }
    Ok((output, count))
}

fn push(
    output: &mut [HidKeyTransition; MAX_TRANSITIONS_PER_REPORT],
    count: &mut usize,
    transition: HidKeyTransition,
) -> Result<(), HidError> {
    let slot = output.get_mut(*count).ok_or(HidError::TransitionOverflow)?;
    *slot = transition;
    *count += 1;
    Ok(())
}

pub(super) fn retain_transition(
    output: &mut [HidKeyTransition; 2],
    count: &mut usize,
    transition: HidKeyTransition,
) -> Result<(), HidError> {
    let slot = output.get_mut(*count).ok_or(HidError::TransitionOverflow)?;
    *slot = transition;
    *count += 1;
    Ok(())
}
