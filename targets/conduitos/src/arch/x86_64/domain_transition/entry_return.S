    // Proof-only stamps use no flag-changing instructions. EAX/EDX are
    // preserved by each caller or retained in R8/R9 at common Root return.
    .macro envelope_stamp offset
    lfence
    rdtsc
    lfence
    mov dword ptr [rip + conduitos_trap_observation + \offset], eax
    mov dword ptr [rip + conduitos_trap_observation + \offset + 4], edx
    .endm
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
    .if {proof_envelope}
    .global conduitos_trap_observation
conduitos_trap_observation: .zero 112
    .global conduitos_boundary_clock
conduitos_boundary_clock: .zero 40
    .global conduitos_irq_clock
conduitos_irq_clock: .zero 16
    .endif
    .text
    .global conduitos_ordinary_enter
conduitos_ordinary_enter:
    mov r10d, 0
    jmp conduitos_ordinary_entry_common
    .global conduitos_ordinary_resume
conduitos_ordinary_resume:
    mov r10d, 1
conduitos_ordinary_entry_common:
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
    .if {proof_envelope}
    // Root arguments are now parked; no guest authority is added.
    mov qword ptr [rip + conduitos_trap_observation], 1
    mov qword ptr [rip + conduitos_trap_observation + 8], 0
    mov qword ptr [rip + conduitos_trap_observation + 32], 0
    mov qword ptr [rip + conduitos_trap_observation + 40], 0
    envelope_stamp 64
    envelope_stamp 72
    envelope_stamp 16
    .endif
    mov rax, cr4
    mov [rip + conduitos_ordinary_root_cr4], rax
    or rax, 4
    .if {proof_envelope}
    cmp qword ptr [rip + conduitos_boundary_clock], 1
    jne 81f
    and rax, -5
81:
    .endif
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
    test r10d, r10d
    jnz conduitos_ordinary_restore_user
    .if {proof_envelope}
    envelope_stamp 24
    .endif
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
    .if {proof_envelope}
    cmp qword ptr [rip + conduitos_boundary_clock], 1
    jne 82f
    lfence
    rdtsc
    lfence
    mov dword ptr [rip + conduitos_boundary_clock + 8], eax
    mov dword ptr [rip + conduitos_boundary_clock + 12], edx
    xor eax, eax
    xor edx, edx
82:
    .endif
    iretq

    .global conduitos_ordinary_gate
conduitos_ordinary_gate:
    // A gate is only callable from the current user domain.
    test byte ptr [rsp + 8], 3
    jz conduitos_ordinary_root_fault
    .if {proof_envelope}
    push rax
    push rdx
    cmp qword ptr [rip + conduitos_boundary_clock], 1
    jne 83f
    lfence
    rdtsc
    lfence
    mov dword ptr [rip + conduitos_boundary_clock + 32], eax
    mov dword ptr [rip + conduitos_boundary_clock + 36], edx
    mov [rip + conduitos_boundary_clock + 16], r12
    mov [rip + conduitos_boundary_clock + 24], r13
    mov qword ptr [rip + conduitos_boundary_clock], 2
83:
    envelope_stamp 32
    mov qword ptr [rip + conduitos_trap_observation + 8], 1
    pop rdx
    pop rax
    .endif
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
    .if {proof_envelope}
    push rax
    push rdx
    envelope_stamp 32
    mov qword ptr [rip + conduitos_trap_observation + 8], 2
    pop rdx
    pop rax
    .endif
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
    .if {proof_envelope}
    test byte ptr [rsp + 16], 3
    jz conduitos_irq_start_unobserved
    push rax
    push rdx
    lfence
    rdtsc
    lfence
    mov dword ptr [rip + conduitos_irq_clock], eax
    mov dword ptr [rip + conduitos_irq_clock + 4], edx
    pop rdx
    pop rax
conduitos_irq_start_unobserved:
    .endif
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
    // The complete GPR/hardware frame remains at R15. Spill actual guest
    // segment state BEFORE any Root normalization or Rust call.
    mov r15, rsp
    sub rsp, 24
    mov r14, rsp
    test byte ptr [r15 + 136], 3
    jz conduitos_ordinary_irq_root_ready
    mov ax, ds
    mov [r14], ax
    mov ax, es
    mov [r14 + 2], ax
    mov ax, fs
    mov [r14 + 4], ax
    mov ax, gs
    mov [r14 + 6], ax
    mov ecx, 0xc0000100
    rdmsr
    mov [r14 + 8], eax
    mov [r14 + 12], edx
    mov ecx, 0xc0000101
    rdmsr
    mov [r14 + 16], eax
    mov [r14 + 20], edx
    // Root's exact pre-entry selectors/bases remain in existing Root custody.
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
conduitos_ordinary_irq_root_ready:
    // Hardware saved user AC/DF in the IRET frame. Neither may leak to Rust.
    pushfq
    pop rax
    and rax, -263169
    push rax
    popfq
    mov rdi, [r15 + 136]
    mov rsi, [r15 + 120]
    mov rdx, r15
    mov rcx, r14
    and rsp, -16
    call conduitos_ordinary_timer_handler
    mov rsp, r15
    test rax, rax
    jz conduitos_ordinary_timer_resume
    cmp rax, 2
    mov eax, 4
    jne conduitos_ordinary_irq_terminal
    mov eax, 5
conduitos_ordinary_irq_terminal:
    mov edx, 2
    .if {proof_envelope}
    push rax
    push rdx
    lfence
    rdtsc
    lfence
    mov dword ptr [rip + conduitos_irq_clock + 8], eax
    mov dword ptr [rip + conduitos_irq_clock + 12], edx
    pop rdx
    pop rax
    .endif
    jmp conduitos_ordinary_return
conduitos_ordinary_timer_resume:
    // Rust used Root selectors/bases; restore captured user state before GPRs.
    test byte ptr [r15 + 136], 3
    jz conduitos_ordinary_irq_pop
    mov ax, [r14]
    mov ds, ax
    mov ax, [r14 + 2]
    mov es, ax
    mov ax, [r14 + 4]
    mov fs, ax
    mov ax, [r14 + 6]
    mov gs, ax
    mov eax, [r14 + 8]
    mov edx, [r14 + 12]
    mov ecx, 0xc0000100
    wrmsr
    mov eax, [r14 + 16]
    mov edx, [r14 + 20]
    mov ecx, 0xc0000101
    wrmsr
conduitos_ordinary_irq_pop:
    mov rsp, r15
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
    .if {proof_envelope}
    test byte ptr [rsp + 16], 3
    jz conduitos_irq_end_unobserved
    push rax
    push rdx
    lfence
    rdtsc
    lfence
    mov dword ptr [rip + conduitos_irq_clock + 8], eax
    mov dword ptr [rip + conduitos_irq_clock + 12], edx
    pop rdx
    pop rax
conduitos_irq_end_unobserved:
    .endif
    add rsp, 8
    iretq

conduitos_ordinary_return:
    cli
    mov r8, rax
    mov r9, rdx
    .if {proof_envelope}
    envelope_stamp 80
    envelope_stamp 88
    envelope_stamp 48
    .endif
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
    .if {proof_envelope}
    mov rax, [rsp + 48]
    mov [rip + conduitos_trap_observation + 96], rax
    .endif
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    popfq
    .if {proof_envelope}
    // Do not SHL/OR/TEST here: Root's flags have already been restored.
    envelope_stamp 56
    mov dword ptr [rip + conduitos_trap_observation + 40], eax
    mov dword ptr [rip + conduitos_trap_observation + 44], edx
    pushfq
    pop rax
    mov [rip + conduitos_trap_observation + 104], rax
    mov qword ptr [rip + conduitos_trap_observation], 2
    mov rax, r8
    mov rdx, r9
    .endif
    ret
