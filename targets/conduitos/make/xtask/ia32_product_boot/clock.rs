//! Boundary appliances use guest work time, independently of host scheduling.
use std::process::Command;

#[derive(Clone, Copy)]
pub(crate) enum GuestClock {
    Host,
    InstructionCount,
}

impl GuestClock {
    pub(crate) fn configure(self, command: &mut Command) {
        if matches!(self, Self::InstructionCount) {
            command.args([
                "-icount",
                "shift=0,align=off,sleep=off",
                "-rtc",
                "base=utc,clock=vm",
            ]);
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Host => "host-time",
            Self::InstructionCount => "instruction-count-shift0-sleep-off-rtc-vm",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_product_keeps_default_clock_and_boundary_clock_is_explicit() {
        let mut normal = Command::new("qemu-system-x86_64");
        GuestClock::Host.configure(&mut normal);
        assert_eq!(normal.get_args().count(), 0);
        assert_eq!(GuestClock::Host.label(), "host-time");
        let mut boundary = Command::new("qemu-system-x86_64");
        GuestClock::InstructionCount.configure(&mut boundary);
        assert_eq!(
            boundary.get_args().collect::<Vec<_>>(),
            [
                "-icount",
                "shift=0,align=off,sleep=off",
                "-rtc",
                "base=utc,clock=vm"
            ]
        );
        assert_ne!(
            GuestClock::InstructionCount.label(),
            GuestClock::Host.label()
        );
    }
}
