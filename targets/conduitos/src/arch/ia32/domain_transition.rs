//! Ring-3 entry, origin-checked traps, and restoration of the Root context.
use super::{domain_budget, domain_memory};
use crate::protected_region::DomainRefusal;
use core::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Active;
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
pub(super) struct TransitionReturn {
    pub value: u64,
    pub origin: u64,
}

unsafe extern "C" {
    fn conduitos_ia32_domain_enter(
        entry: u32,
        stack: u32,
        frame: u32,
        cr3: u32,
        root_floating: u32,
        user_floating: u32,
    ) -> u64;
    fn conduitos_ia32_domain_gate();
    fn conduitos_ia32_domain_budget();
    fn conduitos_ia32_domain_source();
    static conduitos_ia32_domain_faults: [u32; 32];
    static mut conduitos_ia32_domain_sep: u32;
}

pub(super) fn enter(
    space: &domain_memory::AddressSpace,
) -> Result<TransitionReturn, DomainRefusal> {
    super::with_interrupts_masked(|| {
        if ACTIVE.swap(true, Ordering::AcqRel) {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let _active = Active;
        unsafe {
            super::super::ia32_domain_gdt::set_trap_stack(space.trap_stack_top());
            super::install_domain_gate(0x80, conduitos_ia32_domain_gate as *const () as u32, 0xee);
            super::install_domain_gate(
                0x28,
                conduitos_ia32_domain_budget as *const () as u32,
                0x8e,
            );
            super::install_domain_gate(
                0x20,
                conduitos_ia32_domain_source as *const () as u32,
                0x8e,
            );
            for vector in [0, 1, 3, 4, 5, 6, 7, 10, 11, 12, 13, 14, 16, 17, 19, 21, 30] {
                let handler = (*core::ptr::addr_of!(conduitos_ia32_domain_faults))[vector];
                super::install_domain_gate(vector as u8, handler, 0x8e);
            }
            conduitos_ia32_domain_sep = u32::from(core::arch::x86::__cpuid(1).edx & (1 << 11) != 0);
        }
        let _budget = domain_budget::Budget::arm()?;
        let (root_floating, user_floating) = space.floating_states();
        let result = unsafe {
            conduitos_ia32_domain_enter(
                space.entry,
                domain_memory::USER_STACK_TOP,
                domain_memory::USER_FRAME,
                space.cr3,
                root_floating,
                user_floating,
            )
        };
        #[cfg(feature = "ordinary-domain-proof")]
        super::super::domain_context_probe::finish()?;
        Ok(TransitionReturn {
            value: result & 0xffff_ffff,
            origin: result >> 32,
        })
    })
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_domain_budget_irq(user: u32) -> u32 {
    #[cfg(feature = "ordinary-domain-proof")]
    super::super::domain_context_probe::observe_irq();
    u32::from(domain_budget::interrupt(user != 0))
}
#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_domain_source_irq(user: u32) -> u32 {
    #[cfg(feature = "ordinary-domain-proof")]
    super::super::domain_context_probe::observe_irq();
    if user != 0 {
        domain_budget::source_interrupt();
    }
    super::conduitos_ia32_irq_handler();
    0
}

core::arch::global_asm!(
    r#"
.section .bss.conduitos_ia32_domain_context,"aw",@nobits
.balign 16
conduitos_ia32_domain_context:
    .zero 64
.global conduitos_ia32_domain_sep
conduitos_ia32_domain_sep:
    .zero 4
.section .text.conduitos_ia32_domain,"ax",@progbits
.global conduitos_ia32_domain_enter
conduitos_ia32_domain_enter:
    pushfd
    cli
    pushad
    mov [conduitos_ia32_domain_context], esp
    mov eax, cr0
    mov [conduitos_ia32_domain_context + 4], eax
    and eax, 0xfffffff3
    mov cr0, eax
    mov eax, cr3
    mov [conduitos_ia32_domain_context + 8], eax
    mov eax, cr4
    mov [conduitos_ia32_domain_context + 12], eax
    mov ax, ds
    mov [conduitos_ia32_domain_context + 32], ax
    mov ax, es
    mov [conduitos_ia32_domain_context + 34], ax
    mov ax, fs
    mov [conduitos_ia32_domain_context + 36], ax
    mov ax, gs
    mov [conduitos_ia32_domain_context + 38], ax
    mov ecx, 0xc0000080
    rdmsr
    mov [conduitos_ia32_domain_context + 16], eax
    mov [conduitos_ia32_domain_context + 20], edx
    and eax, 0xfffffffe
    wrmsr
    cmp dword ptr [conduitos_ia32_domain_sep], 0
    je 1f
    mov ecx, 0x174
    rdmsr
    mov [conduitos_ia32_domain_context + 24], eax
    mov [conduitos_ia32_domain_context + 28], edx
    xor eax, eax
    xor edx, edx
    wrmsr
1:
    mov eax, [esp + 56]
    mov [conduitos_ia32_domain_context + 40], eax
    fxsave [eax]
    mov eax, [esp + 60]
    fxrstor [eax]
    mov eax, [conduitos_ia32_domain_context + 12]
    and eax, 0xfffbfe7f
    or eax, 0x624
    mov cr4, eax
    mov eax, [esp + 52]
    mov cr3, eax
    mov eax, [conduitos_ia32_domain_context + 4]
    and eax, 0xfffffff3
    or eax, 0x80010020
    mov cr0, eax
    // Read all Root arguments before clearing GPRs and switching segments.
    mov ecx, [esp + 40]
    mov edx, [esp + 44]
    mov ax, 0x23
    mov ds, ax
    mov es, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax
    push 0x23
    push edx
    push 0x202
    push 0x1b
    push ecx
    xor eax, eax
    xor ebx, ebx
    xor ecx, ecx
    xor edx, edx
    xor esi, esi
    xor edi, edi
    xor ebp, ebp
    iretd

.global conduitos_ia32_domain_gate
conduitos_ia32_domain_gate:
    test dword ptr [esp + 4], 3
    jz conduitos_ia32_domain_root_fault
    xor edx, edx
    jmp conduitos_ia32_domain_return

.macro DOMAIN_FAULT vector, error=0
conduitos_ia32_domain_fault_\vector:
    .if \error
    test dword ptr [esp + 8], 3
    .else
    test dword ptr [esp + 4], 3
    .endif
    jz conduitos_ia32_domain_root_fault
    mov eax, 0x100 + \vector
    mov edx, 1
    jmp conduitos_ia32_domain_return
.endm
DOMAIN_FAULT 0
DOMAIN_FAULT 1
DOMAIN_FAULT 3
DOMAIN_FAULT 4
DOMAIN_FAULT 5
DOMAIN_FAULT 6
DOMAIN_FAULT 7
DOMAIN_FAULT 10, 1
DOMAIN_FAULT 11, 1
DOMAIN_FAULT 12, 1
DOMAIN_FAULT 13, 1
DOMAIN_FAULT 14, 1
DOMAIN_FAULT 16
DOMAIN_FAULT 17, 1
DOMAIN_FAULT 19
DOMAIN_FAULT 21, 1
DOMAIN_FAULT 30, 1

// Both IRQ paths protect interrupted floating state and use Root's control
// state for Rust handlers. The Source handler always retains its own wake.
.macro DOMAIN_IRQ name, handler
.global \name
\name:
    push ds
    push es
    push fs
    push gs
    pushad
    // User DF must never govern Root Rust/string operations.
    cld
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ebx, esp
    sub esp, 544
    and esp, -16
    fxsave [esp]
    // Segment pushes use 16-bit operands: 8 segment bytes + 32 PUSHAD
    // bytes + 4 saved EIP bytes place the interrupted CS at offset 44.
    mov eax, [ebx + 44]
    and eax, 3
    mov [esp + 512], eax
    test eax, eax
    jz 2f
    mov ecx, [conduitos_ia32_domain_context + 40]
    fxrstor [ecx]
2:
    sub esp, 16
    mov [esp], eax
    call \handler
    add esp, 16
    test eax, eax
    jnz 3f
    fxrstor [esp]
    mov esp, ebx
    popad
    pop gs
    pop fs
    pop es
    pop ds
    iretd
3:
    mov eax, 4
    mov edx, 2
    jmp conduitos_ia32_domain_return
.endm
DOMAIN_IRQ conduitos_ia32_domain_budget, conduitos_ia32_domain_budget_irq
DOMAIN_IRQ conduitos_ia32_domain_source, conduitos_ia32_domain_source_irq

conduitos_ia32_domain_return:
    cli
    mov cx, 0x10
    mov ds, cx
    mov es, cx
    mov fs, cx
    mov gs, cx
    mov esp, [conduitos_ia32_domain_context]
    mov [esp + 28], eax
    mov [esp + 20], edx
    mov eax, [conduitos_ia32_domain_context + 40]
    fxrstor [eax]
    .if {verify_floating}
    // Verify restoration before the Rust caller can change caller-saved XMMs.
    // FXSAVE's architectural IA-32 state occupies the first 288 bytes.
    cld
    mov esi, eax
    mov edi, [esp + 60]
    fxsave [edi]
    mov ecx, 72
    repe cmpsd
    je 6f
    mov dword ptr [esp + 28], 0x10f
    mov dword ptr [esp + 20], 3
6:
    .endif
    // The Root code/stack have identical mappings while paging is disabled.
    mov eax, [conduitos_ia32_domain_context + 4]
    mov cr0, eax
    mov eax, [conduitos_ia32_domain_context + 8]
    mov cr3, eax
    mov eax, [conduitos_ia32_domain_context + 12]
    mov cr4, eax
    mov ecx, 0xc0000080
    mov eax, [conduitos_ia32_domain_context + 16]
    mov edx, [conduitos_ia32_domain_context + 20]
    wrmsr
    cmp dword ptr [conduitos_ia32_domain_sep], 0
    je 4f
    mov ecx, 0x174
    mov eax, [conduitos_ia32_domain_context + 24]
    mov edx, [conduitos_ia32_domain_context + 28]
    wrmsr
4:
    mov ax, [conduitos_ia32_domain_context + 32]
    mov ds, ax
    mov ax, [conduitos_ia32_domain_context + 34]
    mov es, ax
    mov ax, [conduitos_ia32_domain_context + 36]
    mov fs, ax
    mov ax, [conduitos_ia32_domain_context + 38]
    mov gs, ax
    popad
    popfd
    ret
conduitos_ia32_domain_root_fault:
    cli
5:  hlt
    jmp 5b

.section .rodata.conduitos_ia32_domain_faults,"a",@progbits
.global conduitos_ia32_domain_faults
conduitos_ia32_domain_faults:
.long conduitos_ia32_domain_fault_0, conduitos_ia32_domain_fault_1, 0, conduitos_ia32_domain_fault_3
.long conduitos_ia32_domain_fault_4, conduitos_ia32_domain_fault_5, conduitos_ia32_domain_fault_6, conduitos_ia32_domain_fault_7
.long 0, 0, conduitos_ia32_domain_fault_10, conduitos_ia32_domain_fault_11
.long conduitos_ia32_domain_fault_12, conduitos_ia32_domain_fault_13, conduitos_ia32_domain_fault_14, 0
.long conduitos_ia32_domain_fault_16, conduitos_ia32_domain_fault_17, 0, conduitos_ia32_domain_fault_19
.long 0, conduitos_ia32_domain_fault_21, 0, 0, 0, 0, 0, 0, 0, 0, conduitos_ia32_domain_fault_30, 0
"#,
    verify_floating = const cfg!(feature = "ordinary-domain-proof") as u32,
);
