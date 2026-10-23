//! Actual live-GPR and DF checks after a finite canary loop interrupted by IRQ.
use core::arch::global_asm;
global_asm!(r#"
.global irq_canary_entry
irq_canary_entry:
    push rdi
    std
    mov rax, 101
    mov rcx, 1000000
    mov rdx, 103
    mov rbx, 104
    mov rbp, 105
    mov rsi, 106
    mov rdi, 107
    mov r8, 108
    mov r9, 109
    mov r10, 110
    mov r11, 111
    mov r12, 112
    mov r13, 113
    mov r14, 114
    mov r15, 115
    clc
irq_canary_loop:
    dec rcx
    jnz irq_canary_loop
    pushfq
    and qword ptr [rsp], 0xed5
    cmp qword ptr [rsp], 0x644
    jne irq_canary_bad_flags
    lea rsp, [rsp + 8]
    cmp rax, 101
    jne irq_canary_bad
    cmp rcx, 0
    jne irq_canary_bad
    cmp rdx, 103
    jne irq_canary_bad
    cmp rbx, 104
    jne irq_canary_bad
    cmp rbp, 105
    jne irq_canary_bad
    cmp rsi, 106
    jne irq_canary_bad
    cmp rdi, 107
    jne irq_canary_bad
    cmp r8, 108
    jne irq_canary_bad
    cmp r9, 109
    jne irq_canary_bad
    cmp r10, 110
    jne irq_canary_bad
    cmp r11, 111
    jne irq_canary_bad
    cmp r12, 112
    jne irq_canary_bad
    cmp r13, 113
    jne irq_canary_bad
    cmp r14, 114
    jne irq_canary_bad
    cmp r15, 115
    jne irq_canary_bad
    pop rdi
    cld
    mov dword ptr [rdi + {status}], 0
    mov dword ptr [rdi + {output_length}], 0
    xor eax, eax
    int 0x80
    ud2
irq_canary_bad_flags:
    lea rsp, [rsp + 8]
irq_canary_bad:
    cld
    mov eax, 3
    int 0x80
    ud2
"#,
status=const core::mem::offset_of!(crate::frame::TextFrame,status),
output_length=const core::mem::offset_of!(crate::frame::TextFrame,output_length),
);
