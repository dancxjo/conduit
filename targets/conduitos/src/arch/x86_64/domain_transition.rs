//! Ordinary CPL3 entry, terminal gate, and hard timer return to Root.
use super::{domain_budget, domain_memory, gdt, idt};
use crate::protected_region::DomainRefusal;
use core::{
    arch::global_asm,
    sync::atomic::{AtomicBool, Ordering},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
#[repr(C)]
pub(super) struct TransitionReturn {
    pub value: u64,
    pub origin: u64,
}
unsafe extern "C" {
    fn conduitos_ordinary_enter(entry: u64, stack: u64, frame: u64, cr3: u64) -> TransitionReturn;
    fn conduitos_ordinary_gate();
    fn conduitos_ordinary_gp();
    fn conduitos_ordinary_pf();
    fn conduitos_ordinary_ud();
    fn conduitos_ordinary_nm();
    fn conduitos_ordinary_exception_0();
    fn conduitos_ordinary_exception_1();
    fn conduitos_ordinary_exception_3();
    fn conduitos_ordinary_exception_4();
    fn conduitos_ordinary_exception_5();
    fn conduitos_ordinary_exception_10();
    fn conduitos_ordinary_exception_11();
    fn conduitos_ordinary_exception_12();
    fn conduitos_ordinary_exception_16();
    fn conduitos_ordinary_exception_17();
    fn conduitos_ordinary_exception_19();
    fn conduitos_ordinary_exception_21();
    fn conduitos_ordinary_exception_30();
    fn conduitos_ordinary_timer();
    fn conduitos_ordinary_budget();
}

pub(super) fn enter(
    space: &domain_memory::AddressSpace,
) -> Result<TransitionReturn, DomainRefusal> {
    let _interrupts = super::cpu::InterruptMask::new();
    if ACTIVE.swap(true, Ordering::AcqRel) {
        return Err(DomainRefusal::InvalidLifecycle);
    }
    unsafe {
        gdt::set_ring0_stack(space.trap_stack_top());
        idt::install_handler(
            0x80,
            conduitos_ordinary_gate as *const () as u64,
            idt::USER_INTERRUPT_GATE,
        );
        idt::install_handler(13, conduitos_ordinary_gp as *const () as u64, 0x8e);
        idt::install_handler(14, conduitos_ordinary_pf as *const () as u64, 0x8e);
        idt::install_handler(6, conduitos_ordinary_ud as *const () as u64, 0x8e);
        idt::install_handler(7, conduitos_ordinary_nm as *const () as u64, 0x8e);
        idt::install_handler(0, conduitos_ordinary_exception_0 as *const () as u64, 0x8e);
        idt::install_handler(1, conduitos_ordinary_exception_1 as *const () as u64, 0x8e);
        idt::install_handler(3, conduitos_ordinary_exception_3 as *const () as u64, 0x8e);
        idt::install_handler(4, conduitos_ordinary_exception_4 as *const () as u64, 0x8e);
        idt::install_handler(5, conduitos_ordinary_exception_5 as *const () as u64, 0x8e);
        idt::install_handler(
            10,
            conduitos_ordinary_exception_10 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            11,
            conduitos_ordinary_exception_11 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            12,
            conduitos_ordinary_exception_12 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            16,
            conduitos_ordinary_exception_16 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            17,
            conduitos_ordinary_exception_17 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            19,
            conduitos_ordinary_exception_19 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            21,
            conduitos_ordinary_exception_21 as *const () as u64,
            0x8e,
        );
        idt::install_handler(
            30,
            conduitos_ordinary_exception_30 as *const () as u64,
            0x8e,
        );

        idt::install_handler(
            super::TIMER_IRQ_VECTOR,
            conduitos_ordinary_timer as *const () as u64,
            0x8e,
        );
    }
    unsafe {
        idt::install_handler(
            domain_budget::VECTOR,
            conduitos_ordinary_budget as *const () as u64,
            0x8e,
        );
    }
    let budget = match domain_budget::Budget::arm() {
        Ok(budget) => budget,
        Err(error) => {
            ACTIVE.store(false, Ordering::Release);
            return Err(error);
        }
    };
    let result = unsafe {
        conduitos_ordinary_enter(
            space.entry,
            domain_memory::USER_STACK_TOP - 8,
            domain_memory::USER_FRAME,
            space.cr3,
        )
    };
    drop(budget);
    ACTIVE.store(false, Ordering::Release);
    Ok(result)
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ordinary_timer_handler(user: u64, budget: u64) -> u64 {
    if budget != 0 {
        u64::from(domain_budget::interrupt(
            user & 3 == 3 && ACTIVE.load(Ordering::Acquire),
        ))
    } else {
        if user & 3 == 3 {
            domain_budget::source_interrupt();
        }
        // Source timer facts remain Source facts even while a domain is running.
        super::irq::conduitos_timer_irq_handler();
        0
    }
}

global_asm!(
    r#"
    .bss
    .balign 8
conduitos_ordinary_root_rsp: .quad 0
conduitos_ordinary_root_cr3: .quad 0
conduitos_ordinary_root_cr0: .quad 0
conduitos_ordinary_root_cr4: .quad 0
conduitos_ordinary_root_segments: .quad 0
conduitos_ordinary_root_ss: .quad 0
conduitos_ordinary_root_fs: .quad 0
conduitos_ordinary_root_gs: .quad 0
    .text
    .global conduitos_ordinary_enter
conduitos_ordinary_enter:
    pushfq
    cli
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rip + conduitos_ordinary_root_rsp], rsp
    mov rax, cr3
    mov [rip + conduitos_ordinary_root_cr3], rax
    mov cr3, rcx
    mov r12, rdi
    mov r13, rsi
    mov r14, rdx
    mov rax, cr4
    mov [rip + conduitos_ordinary_root_cr4], rax
    or rax, 4
    and rax, -65793
    mov cr4, rax
    mov ax, ss
    mov [rip + conduitos_ordinary_root_ss], ax
    mov ax, ds
    mov [rip + conduitos_ordinary_root_segments], ax
    mov ax, es
    mov [rip + conduitos_ordinary_root_segments + 2], ax
    mov ax, fs
    mov [rip + conduitos_ordinary_root_segments + 4], ax
    mov ax, gs
    mov [rip + conduitos_ordinary_root_segments + 6], ax
    mov ecx, 0xc0000100
    rdmsr
    mov [rip + conduitos_ordinary_root_fs], eax
    mov [rip + conduitos_ordinary_root_fs + 4], edx
    mov ecx, 0xc0000101
    rdmsr
    mov [rip + conduitos_ordinary_root_gs], eax
    mov [rip + conduitos_ordinary_root_gs + 4], edx
    mov ax, {user_data}
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    xor eax, eax
    xor edx, edx
    mov ecx, 0xc0000100
    wrmsr
    mov ecx, 0xc0000101
    wrmsr
    // The admitted soft-float implementation has no floating-point authority.
    // Trap FPU/SIMD use rather than exposing inherited Root register contents.
    mov rax, cr0
    mov [rip + conduitos_ordinary_root_cr0], rax
    or rax, 8
    mov cr0, rax
    mov rax, r12
    mov rdi, r14
    push {user_data}
    push r13
    push 0x202
    push {user_code}
    push rax
    xor eax, eax
    xor ebx, ebx
    xor ecx, ecx
    xor edx, edx
    xor esi, esi
    xor ebp, ebp
    xor r8d, r8d
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
    xor r12d, r12d
    xor r13d, r13d
    xor r14d, r14d
    xor r15d, r15d
    iretq

    .global conduitos_ordinary_gate
conduitos_ordinary_gate:
    // A gate is only callable from the current user domain.
    test byte ptr [rsp + 8], 3
    jz conduitos_ordinary_root_fault
    xor edx, edx
    jmp conduitos_ordinary_return

    .global conduitos_ordinary_gp
conduitos_ordinary_gp:
    mov eax, 13
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_pf
conduitos_ordinary_pf:
    mov eax, 14
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_ud
conduitos_ordinary_ud:
    push 0
    mov eax, 6
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_nm
conduitos_ordinary_nm:
    push 0
    mov eax, 7
    .global conduitos_ordinary_exception_0
conduitos_ordinary_exception_0:
    push 0
    mov eax, 0
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_1
conduitos_ordinary_exception_1:
    push 0
    mov eax, 1
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_3
conduitos_ordinary_exception_3:
    push 0
    mov eax, 3
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_4
conduitos_ordinary_exception_4:
    push 0
    mov eax, 4
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_5
conduitos_ordinary_exception_5:
    push 0
    mov eax, 5
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_10
conduitos_ordinary_exception_10:
    mov eax, 10
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_11
conduitos_ordinary_exception_11:
    mov eax, 11
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_12
conduitos_ordinary_exception_12:
    mov eax, 12
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_16
conduitos_ordinary_exception_16:
    push 0
    mov eax, 16
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_17
conduitos_ordinary_exception_17:
    mov eax, 17
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_19
conduitos_ordinary_exception_19:
    push 0
    mov eax, 19
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_21
conduitos_ordinary_exception_21:
    mov eax, 21
    jmp conduitos_ordinary_fault
    .global conduitos_ordinary_exception_30
conduitos_ordinary_exception_30:
    mov eax, 30
    jmp conduitos_ordinary_fault
conduitos_ordinary_fault:
    test byte ptr [rsp + 16], 3
    jz conduitos_ordinary_root_fault
    add rax, 0x100
    mov edx, 1
    jmp conduitos_ordinary_return
conduitos_ordinary_root_fault:
    mov rdi, rax
    and rsp, -16
    call conduitos_exception_handler
    ud2

    .global conduitos_ordinary_timer
conduitos_ordinary_timer:
    push 0
    jmp conduitos_ordinary_irq
    .global conduitos_ordinary_budget
conduitos_ordinary_budget:
    push 1
conduitos_ordinary_irq:
    push rax
    push rcx
    push rdx
    push rbx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov r15, rsp
    mov rdi, [rsp + 136]
    mov rsi, [rsp + 120]
    and rsp, -16
    call conduitos_ordinary_timer_handler
    mov rsp, r15
    test rax, rax
    jz conduitos_ordinary_timer_resume
    mov eax, 4
    mov edx, 2
    jmp conduitos_ordinary_return
conduitos_ordinary_timer_resume:
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rbp
    pop rbx
    pop rdx
    pop rcx
    pop rax
    add rsp, 8
    iretq

conduitos_ordinary_return:
    cli
    mov r8, rax
    mov r9, rdx
    mov rcx, [rip + conduitos_ordinary_root_cr4]
    mov cr4, rcx
    mov ax, [rip + conduitos_ordinary_root_ss]
    mov ss, ax
    mov ax, [rip + conduitos_ordinary_root_segments]
    mov ds, ax
    mov ax, [rip + conduitos_ordinary_root_segments + 2]
    mov es, ax
    mov ax, [rip + conduitos_ordinary_root_segments + 4]
    mov fs, ax
    mov ax, [rip + conduitos_ordinary_root_segments + 6]
    mov gs, ax
    mov eax, [rip + conduitos_ordinary_root_fs]
    mov edx, [rip + conduitos_ordinary_root_fs + 4]
    mov ecx, 0xc0000100
    wrmsr
    mov eax, [rip + conduitos_ordinary_root_gs]
    mov edx, [rip + conduitos_ordinary_root_gs + 4]
    mov ecx, 0xc0000101
    wrmsr
    mov rax, r8
    mov rdx, r9
    mov rcx, [rip + conduitos_ordinary_root_cr0]
    mov cr0, rcx
    mov rcx, [rip + conduitos_ordinary_root_cr3]
    mov cr3, rcx
    mov rsp, [rip + conduitos_ordinary_root_rsp]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    popfq
    ret
"#,
    user_data = const gdt::USER_DATA_SELECTOR,
    user_code = const gdt::USER_CODE_SELECTOR,
);
