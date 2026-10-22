    // This instrumentation exists only in the ordinary-domain proof appliance.
    // Both controls retain the same serialized read/reconstruction envelope.
    .if {proof_cr3}
    .macro proof_read
    lfence
    rdtsc
    lfence
    shl rdx, 32
    or rax, rdx
    .endm
    .macro proof_control offset
    proof_read
    mov r10, rax
    proof_read
    mov [rip + conduitos_cr3_observation + \offset], r10
    mov [rip + conduitos_cr3_observation + \offset + 8], rax
    .endm
    .bss
    .balign 8
    .global conduitos_cr3_observation
conduitos_cr3_observation:
    .quad 0, 0, 0, 0, 0, 0, 0, 0, 0
    .global conduitos_state_observation
conduitos_state_observation:
    // valid; eight register timestamps; IRQ count/total/min/max/bad/control.
    .quad 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0
    .macro proof_state_control offset
    proof_read
    mov r10, rax
    proof_read
    mov [rip + conduitos_state_observation + \offset], r10
    mov [rip + conduitos_state_observation + \offset + 8], rax
    .endm
    .endif

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
    .if {proof_cr3}
    mov qword ptr [rip + conduitos_state_observation], 0
    mov qword ptr [rip + conduitos_state_observation + 72], 0
    mov qword ptr [rip + conduitos_state_observation + 80], 0
    mov qword ptr [rip + conduitos_state_observation + 88], -1
    mov qword ptr [rip + conduitos_state_observation + 96], 0
    mov qword ptr [rip + conduitos_state_observation + 104], 0
    mov qword ptr [rip + conduitos_state_observation + 112], 0
    mov r11, rdx
    proof_state_control 8
    proof_read
    mov r10, rax
    .endif
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    .if {proof_cr3}
    proof_read
    mov [rip + conduitos_state_observation + 24], r10
    mov [rip + conduitos_state_observation + 32], rax
    mov rdx, r11
    .endif
    mov [rip + conduitos_ordinary_root_rsp], rsp
    mov rax, cr3
    mov [rip + conduitos_ordinary_root_cr3], rax
    .if {proof_cr3}
    mov qword ptr [rip + conduitos_cr3_observation], 0
    // RDX is the admitted frame pointer; RDTSC destroys EDX.
    mov r11, rdx
    proof_control 8
    proof_read
    mov r10, rax
    mov cr3, rcx
    proof_read
    mov [rip + conduitos_cr3_observation + 24], r10
    mov [rip + conduitos_cr3_observation + 32], rax
    mov rdx, r11
    .else
    mov cr3, rcx
    .endif
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
    jmp conduitos_ordinary_fault
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
    cld
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
    // The interrupted User DF must not affect Root Rust/string operations.
    cld
    mov r15, rsp
    .if {proof_cr3}
    // Guest registers are already saved. Rust preserves R12/R13/R15 by ABI.
    mov r13, [r15 + 136]
    and r13, 3
    cmp r13, 3
    jne .Lstate_irq_start_done
    proof_read
    mov r10, rax
    proof_read
    cmp rax, r10
    jb .Lstate_irq_control_bad
    sub rax, r10
    add [rip + conduitos_state_observation + 112], rax
    jnc .Lstate_irq_control_done
.Lstate_irq_control_bad:
    mov qword ptr [rip + conduitos_state_observation + 104], 1
.Lstate_irq_control_done:
    proof_read
    mov r12, rax
.Lstate_irq_start_done:
    .endif
    mov rdi, [rsp + 136]
    mov rsi, [rsp + 120]
    and rsp, -16
    call conduitos_ordinary_timer_handler
    mov rsp, r15
    .if {proof_cr3}
    cmp r13, 3
    jne .Lstate_irq_end_done
    // No collector call may clobber this saved handler decision.
    mov r11, rax
    proof_read
    cmp rax, r12
    jb .Lstate_irq_bad
    sub rax, r12
    add qword ptr [rip + conduitos_state_observation + 72], 1
    jc .Lstate_irq_bad
    add [rip + conduitos_state_observation + 80], rax
    jc .Lstate_irq_bad
    cmp rax, [rip + conduitos_state_observation + 88]
    jae .Lstate_irq_min_done
    mov [rip + conduitos_state_observation + 88], rax
.Lstate_irq_min_done:
    cmp rax, [rip + conduitos_state_observation + 96]
    jbe .Lstate_irq_restore
    mov [rip + conduitos_state_observation + 96], rax
    jmp .Lstate_irq_restore
.Lstate_irq_bad:
    mov qword ptr [rip + conduitos_state_observation + 104], 1
.Lstate_irq_restore:
    mov rax, r11
.Lstate_irq_end_done:
    .endif
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
    .if {proof_cr3}
    // Exact return value/origin remain in R8/R9 through all counter reads.
    proof_control 40
    proof_read
    mov r10, rax
    mov cr3, rcx
    proof_read
    mov [rip + conduitos_cr3_observation + 56], r10
    mov [rip + conduitos_cr3_observation + 64], rax
    mov qword ptr [rip + conduitos_cr3_observation], 1
    mov rax, r8
    mov rdx, r9
    .else
    mov cr3, rcx
    .endif
    mov rsp, [rip + conduitos_ordinary_root_rsp]
    .if {proof_cr3}
    proof_state_control 40
    proof_read
    mov r10, rax
    .endif
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    .if {proof_cr3}
    proof_read
    mov [rip + conduitos_state_observation + 56], r10
    mov [rip + conduitos_state_observation + 64], rax
    mov qword ptr [rip + conduitos_state_observation], 1
    mov rax, r8
    mov rdx, r9
    .endif
    popfq
    ret
