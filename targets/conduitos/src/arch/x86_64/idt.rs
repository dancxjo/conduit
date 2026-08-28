use core::{arch::global_asm, cell::UnsafeCell, mem::size_of};

use super::TIMER_IRQ_VECTOR;

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const INTERRUPT_GATE: u8 = 0x8e;

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct Entry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl Entry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn handler(address: u64, ist: u8) -> Self {
        Self {
            offset_low: address as u16,
            selector: KERNEL_CODE_SELECTOR,
            ist,
            attributes: INTERRUPT_GATE,
            offset_middle: (address >> 16) as u16,
            offset_high: (address >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, align(16))]
struct Table([Entry; 256]);

struct SharedTable(UnsafeCell<Table>);

unsafe impl Sync for SharedTable {}

static TABLE: SharedTable = SharedTable(UnsafeCell::new(Table([Entry::MISSING; 256])));

#[repr(C, packed)]
struct Descriptor {
    limit: u16,
    base: u64,
}

unsafe extern "C" {
    fn conduitos_unhandled_exception_stub();
    fn conduitos_invalid_opcode_stub();
    fn conduitos_double_fault_stub();
    fn conduitos_general_protection_stub();
    fn conduitos_page_fault_stub();
    fn conduitos_timer_irq_stub();
}

pub(super) fn initialize() {
    let table = TABLE.0.get();
    unsafe {
        let fallback = Entry::handler(
            conduitos_unhandled_exception_stub as *const () as usize as u64,
            0,
        );
        (*table).0.fill(fallback);
        (*table).0[6] = Entry::handler(
            conduitos_invalid_opcode_stub as *const () as usize as u64,
            0,
        );
        (*table).0[8] = Entry::handler(conduitos_double_fault_stub as *const () as usize as u64, 1);
        (*table).0[13] = Entry::handler(
            conduitos_general_protection_stub as *const () as usize as u64,
            0,
        );
        (*table).0[14] = Entry::handler(conduitos_page_fault_stub as *const () as usize as u64, 0);
        (*table).0[usize::from(TIMER_IRQ_VECTOR)] =
            Entry::handler(conduitos_timer_irq_stub as *const () as usize as u64, 0);
        let descriptor = Descriptor {
            limit: (size_of::<Table>() - 1) as u16,
            base: core::ptr::addr_of!(*table) as u64,
        };
        core::arch::asm!("lidt [{}]", in(reg) &descriptor, options(readonly, nostack));
    }
}

global_asm!(
    r#"
    .global conduitos_unhandled_exception_stub
conduitos_unhandled_exception_stub:
    mov rdi, 255
    jmp conduitos_exception_common

    .global conduitos_invalid_opcode_stub
conduitos_invalid_opcode_stub:
    mov rdi, 6
    jmp conduitos_exception_common

    .global conduitos_double_fault_stub
conduitos_double_fault_stub:
    mov rdi, 8
    jmp conduitos_exception_common

    .global conduitos_general_protection_stub
conduitos_general_protection_stub:
    mov rdi, 13
    jmp conduitos_exception_common

    .global conduitos_page_fault_stub
conduitos_page_fault_stub:
    mov rdi, 14
    jmp conduitos_exception_common

conduitos_exception_common:
    cli
    and rsp, -16
    call conduitos_exception_handler
    ud2

    .global conduitos_timer_irq_stub
conduitos_timer_irq_stub:
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
    and rsp, -16
    call conduitos_timer_irq_handler
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
    iretq
"#
);
