//! Proof-only exact assembly payload. U1/U2 remain in reserved R12/R13;
//! no extra guest mapping, exchange words or compiler prologue are introduced.
use core::arch::global_asm;
global_asm!(r#"
.global domain_entry
domain_entry:
    cmp dword ptr [rdi + {probe}], 22
    je boundary_clock_entry
    cmp dword ptr [rdi + {probe}], 23
    je irq_canary_entry
    jmp domain_rust_entry
boundary_clock_entry:
    lfence
    rdtsc
    lfence
    shl rdx, 32
    or rax, rdx
    mov r12, rax
    mov ecx, [rdi + {input_length}]
    cmp ecx, 256
    ja boundary_clock_bad
    cmp dword ptr [rdi + {capacity}], 256
    jne boundary_clock_bad
    xor esi, esi
boundary_clock_loop:
    cmp esi, ecx
    jae boundary_clock_done
    mov al, [rdi + rsi + {input}]
    cmp al, 97
    jb boundary_clock_store
    cmp al, 122
    ja boundary_clock_store
    sub al, 32
boundary_clock_store:
    mov [rdi + rsi + {output}], al
    inc esi
    jmp boundary_clock_loop
boundary_clock_done:
    mov [rdi + {output_length}], ecx
    mov dword ptr [rdi + {status}], 0
    lfence
    rdtsc
    lfence
    shl rdx, 32
    or rax, rdx
    mov r13, rax
    xor eax, eax
    int 0x80
    ud2
boundary_clock_bad:
    mov eax, 3
    int 0x80
    ud2
"#,
probe=const core::mem::offset_of!(crate::frame::TextFrame,probe),
input_length=const core::mem::offset_of!(crate::frame::TextFrame,input_length),
output_length=const core::mem::offset_of!(crate::frame::TextFrame,output_length),
capacity=const core::mem::offset_of!(crate::frame::TextFrame,capacity),
status=const core::mem::offset_of!(crate::frame::TextFrame,status),
input=const core::mem::offset_of!(crate::frame::TextFrame,input),
output=const core::mem::offset_of!(crate::frame::TextFrame,output),
);
