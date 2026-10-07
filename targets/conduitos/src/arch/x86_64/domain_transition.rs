//! Ordinary CPL3 entry, terminal gate, and hard timer return to Root.
use super::{domain_memory, gdt, idt, interrupt_controller};
use crate::protected_region::DomainRefusal;
use core::{
    arch::global_asm,
    sync::atomic::{AtomicBool, Ordering},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
unsafe extern "C" {
    fn conduitos_ordinary_enter(entry: u64, stack: u64, frame: u64, cr3: u64) -> u64;
    fn conduitos_ordinary_gate();
    fn conduitos_ordinary_gp();
    fn conduitos_ordinary_pf();
    fn conduitos_ordinary_ud();
    fn conduitos_ordinary_nm();
    fn conduitos_ordinary_timer();
}

pub(super) fn enter(space: &domain_memory::AddressSpace) -> Result<u64, DomainRefusal> {
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
        idt::install_handler(
            super::TIMER_IRQ_VECTOR,
            conduitos_ordinary_timer as *const () as u64,
            0x8e,
        );
    }
    // This profile reserves one hard timer slice for a computation entry. A
    // domain cannot disable this interrupt and never receives timer authority.
    interrupt_controller::arm_timer();
    let result = unsafe {
        conduitos_ordinary_enter(
            space.entry,
            domain_memory::USER_STACK_TOP - 8,
            domain_memory::USER_FRAME,
            space.cr3,
        )
    };
    interrupt_controller::cancel_timer();
    ACTIVE.store(false, Ordering::Release);
    Ok(result)
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_ordinary_timer_handler(user: u64) -> u64 {
    if user & 3 == 3 && ACTIVE.load(Ordering::Acquire) {
        interrupt_controller::end_timer_interrupt();
        1
    } else {
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
    // The admitted soft-float implementation has no floating-point authority.
    // Trap FPU/SIMD use rather than exposing inherited Root register contents.
    mov rax, cr0
    mov [rip + conduitos_ordinary_root_cr0], rax
    or rax, 8
    mov cr0, rax
    mov rax, rdi
    mov rdi, rdx
    push {user_data}
    push rsi
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
conduitos_ordinary_fault:
    test byte ptr [rsp + 16], 3
    jz conduitos_ordinary_root_fault
    add rax, 0x100
    jmp conduitos_ordinary_return
conduitos_ordinary_root_fault:
    mov rdi, rax
    and rsp, -16
    call conduitos_exception_handler
    ud2

    .global conduitos_ordinary_timer
conduitos_ordinary_timer:
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
    mov rdi, [rsp + 128]
    and rsp, -16
    call conduitos_ordinary_timer_handler
    mov rsp, r15
    test rax, rax
    jz conduitos_ordinary_timer_resume
    mov eax, 4
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
    iretq

conduitos_ordinary_return:
    cli
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
