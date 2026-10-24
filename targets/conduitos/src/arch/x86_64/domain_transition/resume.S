// R12 points to a validated, owner-borrowed UserContext for this exact CR3.
// The common entry prologue has already saved Root state and applied TS/TSD /
// PCE/FSGSBASE denial. Offsets are asserted in continuation.rs.
conduitos_ordinary_restore_user:
    mov r15, r12
    push qword ptr [r15 + 152] // SS
    push qword ptr [r15 + 144] // RSP
    push qword ptr [r15 + 136] // RFLAGS, including captured AC/DF/RF
    push qword ptr [r15 + 128] // CS
    push qword ptr [r15 + 120] // RIP
    mov ax, [r15 + 160]
    mov ds, ax
    mov ax, [r15 + 162]
    mov es, ax
    mov ax, [r15 + 164]
    mov fs, ax
    mov ax, [r15 + 166]
    mov gs, ax
    mov eax, [r15 + 168]
    mov edx, [r15 + 172]
    mov ecx, 0xc0000100
    wrmsr
    mov eax, [r15 + 176]
    mov edx, [r15 + 180]
    mov ecx, 0xc0000101
    wrmsr
    mov rax, [r15 + 112]
    mov rcx, [r15 + 104]
    mov rdx, [r15 + 96]
    mov rbx, [r15 + 88]
    mov rbp, [r15 + 80]
    mov rsi, [r15 + 72]
    mov rdi, [r15 + 64]
    mov r8, [r15 + 56]
    mov r9, [r15 + 48]
    mov r10, [r15 + 40]
    mov r11, [r15 + 32]
    mov r12, [r15 + 24]
    mov r13, [r15 + 16]
    mov r14, [r15 + 8]
    mov r15, [r15]
    iretq
