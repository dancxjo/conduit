use core::{arch::asm, cell::UnsafeCell, mem::size_of};

const KERNEL_CODE_SELECTOR: u16 = 0x08;
const KERNEL_DATA_SELECTOR: u16 = 0x10;
const TSS_SELECTOR: u16 = 0x18;
const IST_STACK_BYTES: usize = 16 * 1024;

#[repr(C, packed)]
struct TaskStateSegment {
    reserved_0: u32,
    rsp: [u64; 3],
    reserved_1: u64,
    ist: [u64; 7],
    reserved_2: u64,
    reserved_3: u16,
    iomap_base: u16,
}

impl TaskStateSegment {
    const fn new() -> Self {
        Self {
            reserved_0: 0,
            rsp: [0; 3],
            reserved_1: 0,
            ist: [0; 7],
            reserved_2: 0,
            reserved_3: 0,
            iomap_base: size_of::<Self>() as u16,
        }
    }
}

#[repr(C, align(16))]
struct InterruptStack([u8; IST_STACK_BYTES]);

#[repr(C, align(16))]
struct GdtState {
    entries: [u64; 5],
    tss: TaskStateSegment,
    interrupt_stack: InterruptStack,
}

impl GdtState {
    const fn new() -> Self {
        Self {
            entries: [0, 0x00af_9a00_0000_ffff, 0x00cf_9200_0000_ffff, 0, 0],
            tss: TaskStateSegment::new(),
            interrupt_stack: InterruptStack([0; IST_STACK_BYTES]),
        }
    }
}

struct SharedState(UnsafeCell<GdtState>);

unsafe impl Sync for SharedState {}

static STATE: SharedState = SharedState(UnsafeCell::new(GdtState::new()));

#[repr(C, packed)]
struct Descriptor {
    limit: u16,
    base: u64,
}

pub(super) fn initialize() {
    let state = STATE.0.get();
    unsafe {
        let stack_start = core::ptr::addr_of!((*state).interrupt_stack.0) as u64;
        (*state).tss.ist[0] = stack_start + IST_STACK_BYTES as u64;
        install_tss_descriptor(
            &mut (*state).entries,
            core::ptr::addr_of!((*state).tss) as u64,
        );
        let descriptor = Descriptor {
            limit: (size_of::<[u64; 5]>() - 1) as u16,
            base: core::ptr::addr_of!((*state).entries) as u64,
        };
        asm!("lgdt [{}]", in(reg) &descriptor, options(readonly, nostack));
        asm!(
            "push {code}",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, {data:x}",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            code = const KERNEL_CODE_SELECTOR,
            data = in(reg) KERNEL_DATA_SELECTOR,
            out("rax") _,
        );
        asm!("ltr {selector:x}", selector = in(reg) TSS_SELECTOR, options(nostack));
    }
}

fn install_tss_descriptor(entries: &mut [u64; 5], base: u64) {
    let limit = (size_of::<TaskStateSegment>() - 1) as u64;
    entries[3] = (limit & 0xffff)
        | ((base & 0xffff) << 16)
        | (((base >> 16) & 0xff) << 32)
        | (0x89 << 40)
        | (((limit >> 16) & 0x0f) << 48)
        | (((base >> 24) & 0xff) << 56);
    entries[4] = base >> 32;
}
