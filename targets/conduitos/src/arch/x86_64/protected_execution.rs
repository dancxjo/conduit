//! x86_64 ring-3 proof for the ConduitOS protection-domain contract.

use core::{arch::asm, cell::UnsafeCell, ptr::copy_nonoverlapping};

use crate::{
    boot,
    machine::SerialBase,
    protection_domain::{
        KernelCapabilityHandle, KernelCapabilityRefusal, KernelCapabilityScope,
        KernelCapabilityTable, KernelOperationClaim, KernelRevocationCause, ProtectionDomainId,
    },
};

use super::{Serial, cpu, gdt, idt};

const USER_TEXT: u64 = 0x0040_0000;
const USER_DATA: u64 = USER_TEXT + 0x1000;
const USER_STACK: u64 = USER_TEXT + 0x3000;
const DOMAIN: ProtectionDomainId = ProtectionDomainId(1);
const SIBLING: ProtectionDomainId = ProtectionDomainId(2);
const OPERATION: u32 = 7;
const TRAP_VECTOR: u8 = 0x80;

#[repr(C, align(4096))]
struct Page([u64; 512]);

#[repr(C, align(4096))]
struct Bytes([u8; 4096]);

static mut DOMAIN_PML4: Page = Page([0; 512]);
static mut DOMAIN_PDPT: Page = Page([0; 512]);
static mut DOMAIN_PD: Page = Page([0; 512]);
static mut DOMAIN_PT: Page = Page([0; 512]);
static mut DOMAIN_DATA: Bytes = Bytes([0; 4096]);
static mut DOMAIN_STACK: Bytes = Bytes([0; 4096]);
static mut KERNEL_STACK: Bytes = Bytes([0; 4096]);
static mut SIBLING_STATE: Bytes = Bytes([0x5a; 4096]);

struct Shared<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Shared<T> {}

static TABLE: Shared<Option<KernelCapabilityTable>> = Shared(UnsafeCell::new(None));
static STATE: Shared<ProofState> = Shared(UnsafeCell::new(ProofState::new()));

#[derive(Clone, Copy)]
struct ProofState {
    exact: KernelCapabilityScope,
    valid: u64,
    fault_step: u8,
    pure_without_handle: bool,
    kernel_memory_fault: bool,
    capability_table_fault: bool,
    sibling_memory_fault: bool,
    kernel_function_fault: bool,
    mmio_fault: bool,
    io_port_fault: bool,
    arbitrary_refused: bool,
    stolen_refused: bool,
    wrong_operation_refused: bool,
    malformed_pointer_refused: bool,
    excessive_refused: bool,
    authorized_effect: bool,
    stale_refused: bool,
}

impl ProofState {
    const fn new() -> Self {
        Self {
            exact: EMPTY_SCOPE,
            valid: 0,
            fault_step: 0,
            pure_without_handle: false,
            kernel_memory_fault: false,
            capability_table_fault: false,
            sibling_memory_fault: false,
            kernel_function_fault: false,
            mmio_fault: false,
            io_port_fault: false,
            arbitrary_refused: false,
            stolen_refused: false,
            wrong_operation_refused: false,
            malformed_pointer_refused: false,
            excessive_refused: false,
            authorized_effect: false,
            stale_refused: false,
        }
    }
}

const EMPTY_SCOPE: KernelCapabilityScope = KernelCapabilityScope {
    host: [0; 32],
    boot: [0; 32],
    plan: [0; 32],
    play: [0; 32],
    implementation: [0; 32],
    base: [0; 32],
    base_generation: 0,
    resource: [0; 32],
    resource_generation: 0,
    operation: 0,
    subject: [0; 32],
    authority: [0; 32],
    maximum_parameter_bytes: 0,
    maximum_work_units: 0,
    maximum_in_flight: 0,
    maximum_operations: 0,
};

unsafe extern "C" {
    static __conduitos_user_text_start: u8;
    static __conduitos_user_text_end: u8;
    fn conduitos_domain_user_entry();
    fn conduitos_domain_resume_kernel();
    fn conduitos_domain_resume_capability_table();
    fn conduitos_domain_resume_sibling();
    fn conduitos_domain_resume_kernel_function();
    fn conduitos_domain_resume_mmio();
    fn conduitos_domain_resume_io();
    fn conduitos_domain_trap_stub();
    fn conduitos_domain_gp_fault_stub();
    fn conduitos_domain_page_fault_stub();
}

pub fn run_isolation_proof(hhdm: u64, host: [u8; 32], boot_id: [u8; 32]) -> ! {
    super::initialize_machine();
    let exact = scope(host, boot_id, 1);
    let sibling_scope = scope(host, boot_id, 2);
    let mut table =
        KernelCapabilityTable::new(u64::from_le_bytes(boot_id[..8].try_into().unwrap()) | 1)
            .unwrap_or_else(|_| cpu::deterministic_exit(false));
    let valid = table
        .issue(DOMAIN, exact)
        .unwrap_or_else(|_| cpu::deterministic_exit(false));
    let stolen = table
        .issue(SIBLING, sibling_scope)
        .unwrap_or_else(|_| cpu::deterministic_exit(false));
    unsafe {
        (*STATE.0.get()).exact = exact;
        (*STATE.0.get()).valid = valid.raw_for_domain();
        (*TABLE.0.get()) = Some(table);
        DOMAIN_DATA.0[..8].copy_from_slice(&stolen.raw_for_domain().to_le_bytes());
        DOMAIN_DATA.0[8..16].copy_from_slice(&valid.raw_for_domain().to_le_bytes());
        DOMAIN_DATA.0[16..24].copy_from_slice(&(TABLE.0.get() as u64).to_le_bytes());
        DOMAIN_DATA.0[24..32]
            .copy_from_slice(&(core::ptr::addr_of!(SIBLING_STATE.0) as u64).to_le_bytes());
        install_address_space(hhdm);
        let kernel_stack_top = core::ptr::addr_of!(KERNEL_STACK.0) as u64 + 4096;
        gdt::set_ring0_stack(kernel_stack_top);
        idt::install_handler(
            TRAP_VECTOR,
            conduitos_domain_trap_stub as *const () as u64,
            idt::USER_INTERRUPT_GATE,
        );
        idt::install_handler(13, conduitos_domain_gp_fault_stub as *const () as u64, 0x8e);
        idt::install_handler(
            14,
            conduitos_domain_page_fault_stub as *const () as u64,
            0x8e,
        );
        enter_user();
    }
}

fn scope(host: [u8; 32], boot_id: [u8; 32], seed: u8) -> KernelCapabilityScope {
    KernelCapabilityScope {
        host,
        boot: boot_id,
        plan: [seed; 32],
        play: [seed + 1; 32],
        implementation: [seed + 2; 32],
        base: [seed + 3; 32],
        base_generation: 1,
        resource: [seed + 4; 32],
        resource_generation: 1,
        operation: OPERATION,
        subject: [seed + 5; 32],
        authority: [seed + 6; 32],
        maximum_parameter_bytes: 16,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    }
}

unsafe fn install_address_space(hhdm: u64) {
    let text_start = core::ptr::addr_of!(__conduitos_user_text_start) as u64;
    let text_end = core::ptr::addr_of!(__conduitos_user_text_end) as u64;
    if text_end - text_start != 4096 {
        cpu::deterministic_exit(false);
    }
    let current_cr3: u64;
    unsafe { asm!("mov {}, cr3", out(reg) current_cr3, options(nostack, nomem, preserves_flags)) };
    let current = (hhdm + (current_cr3 & !0xfff)) as *const u64;
    unsafe {
        copy_nonoverlapping(
            current,
            core::ptr::addr_of_mut!(DOMAIN_PML4.0) as *mut u64,
            512,
        );
        let pdpt = physical(core::ptr::addr_of!(DOMAIN_PDPT.0) as u64);
        let pd = physical(core::ptr::addr_of!(DOMAIN_PD.0) as u64);
        let pt = physical(core::ptr::addr_of!(DOMAIN_PT.0) as u64);
        let text = physical(text_start);
        let data = physical(core::ptr::addr_of!(DOMAIN_DATA.0) as u64);
        let stack = physical(core::ptr::addr_of!(DOMAIN_STACK.0) as u64);
        DOMAIN_PDPT.0[0] = pd | 0x7;
        DOMAIN_PD.0[2] = pt | 0x7;
        DOMAIN_PT.0[0] = text | 0x5;
        DOMAIN_PT.0[1] = data | 0x7;
        DOMAIN_PT.0[3] = stack | 0x7;
        DOMAIN_PML4.0[0] = pdpt | 0x7;
        let new_cr3 = physical(core::ptr::addr_of!(DOMAIN_PML4.0) as u64);
        asm!("mov cr3, {}", in(reg) new_cr3, options(nostack, preserves_flags));
    }
}

fn physical(virtual_address: u64) -> u64 {
    boot::executable_physical_address(virtual_address)
        .filter(|address| address & 0xfff == 0)
        .unwrap_or_else(|| cpu::deterministic_exit(false))
}

unsafe fn enter_user() -> ! {
    let entry = user_address(conduitos_domain_user_entry as *const () as u64);
    unsafe {
        asm!(
            "push {user_data}", "push {stack}", "push 0x202", "push {user_code}", "push {entry}",
            "iretq",
            user_data = in(reg) u64::from(gdt::USER_DATA_SELECTOR),
            stack = in(reg) USER_STACK + 4096,
            user_code = in(reg) u64::from(gdt::USER_CODE_SELECTOR),
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}

fn user_address(symbol: u64) -> u64 {
    USER_TEXT + symbol - core::ptr::addr_of!(__conduitos_user_text_start) as u64
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_domain_trap_handler(request: u64, raw_handle: u64, pointer: u64) {
    let state = unsafe { &mut *STATE.0.get() };
    let table = unsafe { (*TABLE.0.get()).as_mut().unwrap() };
    match request {
        0 => state.pure_without_handle = raw_handle == 0,
        1 | 2 | 3 | 4 | 6 | 7 => {
            if request == 6 && !(USER_DATA..USER_DATA + 16).contains(&pointer) {
                state.malformed_pointer_refused = true;
                return;
            }
            let mut claim = claim(&state.exact);
            if request == 3 {
                claim.operation += 1;
            }
            let result = table.authorize(
                DOMAIN,
                KernelCapabilityHandle::from_untrusted(raw_handle),
                claim,
            );
            match request {
                1 => {
                    state.arbitrary_refused = result == Err(KernelCapabilityRefusal::UnknownHandle)
                }
                2 => state.stolen_refused = result == Err(KernelCapabilityRefusal::WrongDomain),
                3 => {
                    state.wrong_operation_refused =
                        result == Err(KernelCapabilityRefusal::WrongScope)
                }
                4 => {
                    if let Ok(lease) = result {
                        let mut serial = Serial::new();
                        state.authorized_effect = serial
                            .present(b"protected-domain-authorized-effect")
                            .is_ok()
                            && table.complete(lease).is_ok();
                    }
                }
                7 => state.excessive_refused = result == Err(KernelCapabilityRefusal::Exhausted),
                _ => {}
            }
        }
        5 => {
            let _ = table.revoke_handle(
                DOMAIN,
                KernelCapabilityHandle::from_untrusted(raw_handle),
                KernelRevocationCause::AuthorityRevoked,
            );
        }
        8 => {
            state.stale_refused = table.authorize(
                DOMAIN,
                KernelCapabilityHandle::from_untrusted(raw_handle),
                claim(&state.exact),
            ) == Err(KernelCapabilityRefusal::Revoked);
        }
        0xff => finish(state),
        _ => {}
    }
}

fn claim(scope: &KernelCapabilityScope) -> KernelOperationClaim {
    KernelOperationClaim {
        boot: scope.boot,
        plan: scope.plan,
        play: scope.play,
        base_generation: scope.base_generation,
        resource_generation: scope.resource_generation,
        operation: scope.operation,
        parameter_bytes: 16,
        work_units: 1,
    }
}

fn finish(state: &ProofState) -> ! {
    let sibling_unchanged = unsafe { SIBLING_STATE.0[..32] == [0x5a; 32] };
    let accepted = state.pure_without_handle
        && state.kernel_memory_fault
        && state.capability_table_fault
        && state.sibling_memory_fault
        && state.kernel_function_fault
        && state.mmio_fault
        && state.io_port_fault
        && state.arbitrary_refused
        && state.stolen_refused
        && state.wrong_operation_refused
        && state.malformed_pointer_refused
        && state.excessive_refused
        && state.authorized_effect
        && state.stale_refused
        && sibling_unchanged;
    if accepted {
        super::early_write(b"CONDUIT_PROTECTION_SIGN {\"schema\":\"conduit.conduitos/protection-domain@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"architecture\":\"x86_64\",\"protection_domain_id\":1,\"privilege\":\"ring3\",\"selected_base\":\"serial/present\",\"selected_resource\":\"qemu-com1\",\"capability_required\":true,\"capability_granted\":true,\"kernel_handle_identity\":\"slot-0-generation-1\",\"enforcement_class\":\"x86_64-ring3-page-table-io-bitmap\",\"page_isolation\":true,\"io_bitmap_denied\":true,\"kernel_capability_table\":true,\"authorized_base_effect\":true,\"faults\":[\"kernel-memory\",\"capability-table-write\",\"sibling-memory\",\"kernel-function\",\"device-mmio\",\"io-port\"],\"refusals\":[\"arbitrary-handle\",\"stolen-handle\",\"wrong-operation\",\"operation-capacity\",\"malformed-pointer\",\"revoked-handle\"],\"revocation\":\"authority-revoked\",\"sibling_unchanged\":true,\"dma_isolation\":false,\"driver_isolation\":false,\"bounded\":true}\n");
    }
    cpu::deterministic_exit(accepted)
}

#[unsafe(no_mangle)]
extern "C" fn conduitos_domain_fault_handler(vector: u64, frame: *mut u64) {
    let state = unsafe { &mut *STATE.0.get() };
    let resume = match state.fault_step {
        0 if vector == 14 => {
            state.kernel_memory_fault = true;
            conduitos_domain_resume_kernel as *const () as u64
        }
        1 if vector == 14 => {
            state.capability_table_fault = true;
            conduitos_domain_resume_capability_table as *const () as u64
        }
        2 if vector == 14 => {
            state.sibling_memory_fault = true;
            conduitos_domain_resume_sibling as *const () as u64
        }
        3 if vector == 14 => {
            state.kernel_function_fault = true;
            conduitos_domain_resume_kernel_function as *const () as u64
        }
        4 if vector == 14 => {
            state.mmio_fault = true;
            conduitos_domain_resume_mmio as *const () as u64
        }
        5 if vector == 13 => {
            state.io_port_fault = true;
            conduitos_domain_resume_io as *const () as u64
        }
        _ => cpu::deterministic_exit(false),
    };
    state.fault_step += 1;
    unsafe { *frame.add(1) = user_address(resume) };
}

core::arch::global_asm!(
    r#"
    .section .user_text,"ax"
    .global conduitos_domain_user_entry
conduitos_domain_user_entry:
    xor rbx, rbx
    xor rax, rax
    int 0x80
    mov rax, [0xffffffff80000000]
    .global conduitos_domain_resume_kernel
conduitos_domain_resume_kernel:
    mov rax, [0x401010]
    mov qword ptr [rax], 0
    .global conduitos_domain_resume_capability_table
conduitos_domain_resume_capability_table:
    mov rax, [0x401018]
    mov rax, [rax]
    .global conduitos_domain_resume_sibling
conduitos_domain_resume_sibling:
    mov rax, 0xffffffff80000000
    call rax
    .global conduitos_domain_resume_kernel_function
conduitos_domain_resume_kernel_function:
    mov rax, [0xffffc00000000000]
    .global conduitos_domain_resume_mmio
conduitos_domain_resume_mmio:
    mov dx, 0x3f8
    out dx, al
    .global conduitos_domain_resume_io
conduitos_domain_resume_io:
    mov rax, 1
    mov rbx, 17
    int 0x80
    mov rax, 2
    mov rbx, [0x401000]
    int 0x80
    mov rax, 3
    mov rbx, [0x401008]
    int 0x80
    mov rax, 4
    int 0x80
    mov rax, 7
    int 0x80
    mov rax, 6
    mov rcx, 0xfffffffffffff000
    int 0x80
    mov rax, 5
    int 0x80
    mov rax, 8
    int 0x80
    mov rax, 0xff
    int 0x80
    ud2
    .balign 4096
    .text

    .global conduitos_domain_trap_stub
conduitos_domain_trap_stub:
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax
    mov rdi, rax
    mov rsi, rbx
    mov rdx, rcx
    call conduitos_domain_trap_handler
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11
    iretq

    .global conduitos_domain_gp_fault_stub
conduitos_domain_gp_fault_stub:
    push 13
    jmp conduitos_domain_fault_common
    .global conduitos_domain_page_fault_stub
conduitos_domain_page_fault_stub:
    push 14
conduitos_domain_fault_common:
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax
    mov rdi, [rsp + 88]
    lea rsi, [rsp + 96]
    call conduitos_domain_fault_handler
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11
    add rsp, 16
    iretq
"#
);
