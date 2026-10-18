//! Fixed compiled SSE2 operations with scoped rounding and returned IEEE flags.
//! Native availability is checked during initialization; no software fallback.
use conduit_core::{Binary64Rounding, IeeeBinary64Operation, IeeeBinary64Result, IeeeF64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeIeeeRefusal {
    UnsupportedTarget,
}
pub const fn available() -> bool {
    cfg!(all(target_arch = "x86_64", target_feature = "sse2"))
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2 {
    use super::*;
    use conduit_core::{ieee_binary64_special_case, IeeeBinary64ExceptionFlags};
    use core::arch::asm;

    fn control(rounding: Binary64Rounding) -> u32 {
        let direction = match rounding {
            Binary64Rounding::NearestTiesEven => 0,
            Binary64Rounding::TowardZero => 3,
            Binary64Rounding::TowardPositive => 2,
            Binary64Rounding::TowardNegative => 1,
        };
        // Mask every exception, clear sticky flags, disable FTZ and DAZ.
        0x1f80 | (direction << 13)
    }
    fn result(bits: u64, status: u32) -> IeeeBinary64Result {
        IeeeBinary64Result {
            value: IeeeF64::from_bits(bits),
            flags: IeeeBinary64ExceptionFlags {
                invalid: status & 1 != 0,
                divide_by_zero: status & 4 != 0,
                overflow: status & 8 != 0,
                underflow: status & 16 != 0,
                inexact: status & 32 != 0,
            },
        }
    }

    // The optional probe prefix/suffix execute inside the same assembly block.
    // No Rust instruction can execute with nondefault control state. The shared
    // middle sequence is identical in the production call and preservation probe.
    macro_rules! instruction {
        ($opcode:literal, $left:expr, $right:expr, $control:expr;
         before[$($before:literal),*]; after[$($after:literal),*]; extra[$($extra:tt)*]) => {{
            let mut value = f64::from_bits($left.bits());
            let right_value = f64::from_bits($right.bits());
            let control = $control;
            let mut saved = 0u32;
            let mut status = 0u32;
            // SAFETY: x86_64 SSE2 is the initialized target profile. All pointers
            // address live aligned u32 locals. Control uses only defined SSE2
            // bits and masks all exceptions. Original MXCSR is restored within
            // this block, before Rust can resume or reorder FP instructions.
            unsafe { asm!(
                $($before,)*
                "stmxcsr [{saved}]",
                "ldmxcsr [{control}]",
                $opcode,
                "stmxcsr [{status}]",
                "ldmxcsr [{saved}]",
                $($after,)*
                value = inout(xmm_reg) value,
                right = in(xmm_reg) right_value,
                saved = in(reg) &mut saved,
                control = in(reg) &control,
                status = in(reg) &mut status,
                $($extra)*
                options(nostack),
            ); }
            result(value.to_bits(), status)
        }};
    }
    pub(super) fn execute(
        operation: IeeeBinary64Operation,
        left: IeeeF64,
        right: IeeeF64,
        rounding: Binary64Rounding,
    ) -> IeeeBinary64Result {
        if let Some(result) = ieee_binary64_special_case(operation, left, right) {
            return result;
        }
        let control = control(rounding);
        match operation {
            IeeeBinary64Operation::Add => {
                instruction!("addsd {value}, {right}",left,right,control;before[];after[];extra[])
            }
            IeeeBinary64Operation::Subtract => {
                instruction!("subsd {value}, {right}",left,right,control;before[];after[];extra[])
            }
            IeeeBinary64Operation::Multiply => {
                instruction!("mulsd {value}, {right}",left,right,control;before[];after[];extra[])
            }
            IeeeBinary64Operation::Divide => {
                instruction!("divsd {value}, {right}",left,right,control;before[];after[];extra[])
            }
        }
    }

    #[cfg(test)]
    pub(super) fn probe(
        rounding: Binary64Rounding,
        injected: u32,
    ) -> (IeeeBinary64Result, u32, u32) {
        // Keep reserved bits and optional DAZ clear for every safe probe input.
        let injected = injected & 0xffbf;
        let mut original = 0u32;
        let mut restored = 0u32;
        // Only the assembly block sees this synthetic nondefault prior state.
        // The actual ABI state is restored before returning to any Rust code.
        let result = instruction!("divsd {value}, {right}",
            IeeeF64::from_bits(0x3ff0_0000_0000_0000), IeeeF64::from_bits(0x4008_0000_0000_0000), control(rounding);
            before["stmxcsr [{original}]", "ldmxcsr [{injected}]"];
            after["stmxcsr [{restored}]", "ldmxcsr [{original}]"];
            extra[original=in(reg) &mut original, injected=in(reg) &injected,
                restored=in(reg) &mut restored,]);
        (result, injected, restored)
    }
}

pub fn execute(
    operation: IeeeBinary64Operation,
    left: IeeeF64,
    right: IeeeF64,
    rounding: Binary64Rounding,
) -> Result<IeeeBinary64Result, NativeIeeeRefusal> {
    if !available() {
        return Err(NativeIeeeRefusal::UnsupportedTarget);
    }
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    {
        Ok(sse2::execute(operation, left, right, rounding))
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    {
        let _ = (operation, left, right, rounding);
        Err(NativeIeeeRefusal::UnsupportedTarget)
    }
}

#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
pub fn preservation_probe(
    rounding: Binary64Rounding,
    injected: u32,
) -> (IeeeBinary64Result, u32, u32) {
    sse2::probe(rounding, injected)
}
