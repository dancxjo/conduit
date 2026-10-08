//! Root-owned flat segments and the privilege-return stack for IA-32 domains.

pub(super) const KERNEL_CODE: u16 = 0x08;
pub(super) const KERNEL_DATA: u16 = 0x10;
pub(super) const USER_CODE: u16 = 0x1b;
pub(super) const USER_DATA: u16 = 0x23;
const TASK_SELECTOR: u16 = 0x28;
const TSS_BYTES: usize = 104;

// Architectural TSS offsets are byte offsets, independent of Rust layout.
// The I/O-map offset points beyond the descriptor limit, denying every port
// to a caller whose CPL exceeds IOPL. No user-controlled bitmap is installed.
#[repr(C, align(16))]
struct TaskState([u32; TSS_BYTES / 4]);

impl TaskState {
    const fn new() -> Self {
        let mut words = [0; TSS_BYTES / 4];
        words[2] = KERNEL_DATA as u32;
        words[25] = (TSS_BYTES as u32) << 16;
        Self(words)
    }
}

const fn task_descriptor(base: u32) -> u64 {
    let limit = (TSS_BYTES - 1) as u64;
    limit
        | (((base & 0xffff) as u64) << 16)
        | ((((base >> 16) & 0xff) as u64) << 32)
        | (0x89_u64 << 40)
        | (((base >> 24) as u64) << 56)
}

#[cfg(target_arch = "x86")]
static mut TASK: TaskState = TaskState::new();
#[cfg(target_arch = "x86")]
static mut GDT: [u64; 6] = [
    0,
    0x00cf_9a00_0000_ffff,
    0x00cf_9200_0000_ffff,
    0x00cf_fa00_0000_ffff,
    0x00cf_f200_0000_ffff,
    0,
];

#[cfg(target_arch = "x86")]
core::arch::global_asm!(
    r#"
.section .text.conduitos_ia32_reload_segments,"ax",@progbits
conduitos_ia32_reload_segments:
    push 0x08
    lea eax, [conduitos_ia32_segments_ready]
    push eax
    retf
conduitos_ia32_segments_ready:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax
    ret
"#
);

/// Called once during machine initialization, with interrupts disabled.
#[cfg(target_arch = "x86")]
pub(super) fn initialize() {
    #[repr(C, packed)]
    struct Descriptor {
        limit: u16,
        base: u32,
    }
    unsafe extern "C" {
        fn conduitos_ia32_reload_segments();
    }
    unsafe {
        GDT[5] = task_descriptor(core::ptr::addr_of!(TASK) as u32);
        let descriptor = Descriptor {
            limit: (core::mem::size_of::<[u64; 6]>() - 1) as u16,
            base: core::ptr::addr_of!(GDT) as u32,
        };
        core::arch::asm!("lgdt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags));
        conduitos_ia32_reload_segments();
        core::arch::asm!("ltr {0:x}", in(reg) TASK_SELECTOR, options(nostack, preserves_flags));
    }
}

/// Root chooses this stack before entering a domain; user memory never owns it.
#[cfg(target_arch = "x86")]
pub(super) fn set_trap_stack(top: u32) {
    unsafe { (*core::ptr::addr_of_mut!(TASK)).0[1] = top };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_has_architectural_offsets_and_denies_user_io() {
        let state = TaskState::new();
        assert_eq!(core::mem::size_of_val(&state.0), 104);
        assert_eq!(state.0[1], 0);
        assert_eq!(state.0[2], u32::from(KERNEL_DATA));
        assert_eq!(state.0[25] >> 16, 104);
        assert_eq!(state.0[25] & 0xffff, 0);
    }

    #[test]
    fn task_descriptor_preserves_base_and_bounds_without_task_call_permission() {
        let base = 0x89ab_cdef;
        let descriptor = task_descriptor(base);
        let decoded = ((descriptor >> 16) & 0xffff)
            | (((descriptor >> 32) & 0xff) << 16)
            | (((descriptor >> 56) & 0xff) << 24);
        assert_eq!(decoded, u64::from(base));
        assert_eq!(descriptor & 0xffff, 103);
        assert_eq!((descriptor >> 40) & 0xff, 0x89);
        assert_eq!((descriptor >> 48) & 0xff, 0);
        assert_eq!(KERNEL_CODE & 3, 0);
        assert_eq!(USER_CODE & 3, 3);
        assert_eq!(USER_DATA & 3, 3);
        assert_eq!(TASK_SELECTOR & 3, 0);
    }
}
